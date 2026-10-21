//! Generation-selected status segments with launch-based producer deadlines.

mod command;
mod context;
mod text;

use super::{App, state::TabBarStatusSegment};
use crate::settings::{
    tab_bar::TabBarRightEntryConfig,
    tab_bar::{
        MAX_TAB_BAR_COMMAND_INTERVAL_SECONDS, MAX_TAB_BAR_COMMAND_TIMEOUT_SECONDS,
        MAX_TAB_BAR_RIGHT_ENTRIES, parse_tab_bar_datetime_format,
    },
};
use command::{StatusCommandRequest, StatusCommandTask, spawn_status_command};
use std::time::{Duration, Instant};
use text::{sanitize_literal_text, sanitize_separator, sanitize_status_text};

/// Local calendar refresh cadence.
const DATETIME_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

/// Retained producers and configuration identity for ordered status segments.
#[derive(Default)]
pub(super) struct TabBarStatus {
    /// Identity advanced before old producers are removed.
    generation: u64,
    /// Local calendar producers.
    datetimes: Vec<TabBarDatetimeRuntime>,
    /// Shell command producers with owned task lifetimes.
    commands: Vec<TabBarCommandRuntime>,
    /// Next local calendar refresh, absent without calendar entries.
    next_datetime_refresh: Option<Instant>,
}

/// One calendar format bound to its retained segment.
struct TabBarDatetimeRuntime {
    /// Destination segment.
    segment_index: usize,
    /// Qualified format description.
    format: time::format_description::OwnedFormatItem,
}

/// One shell command's launch policy and active owner.
struct TabBarCommandRuntime {
    /// Destination segment.
    segment_index: usize,
    /// Supplied shell command.
    command: String,
    /// Time between launches.
    interval: Duration,
    /// Maximum execution lifetime measured before first poll.
    timeout: Duration,
    /// Next launch time when idle.
    next_run_at: Instant,
    /// Running task, removed on completion or reload.
    task: Option<StatusCommandTask>,
}

impl<C: FnMut() -> u128> App<C> {
    /// Replaces status producers and invalidates every previous command result.
    pub fn configure_tab_bar_status(
        &mut self,
        entries: &[TabBarRightEntryConfig],
        separator: &str,
    ) {
        self.tab_bar_status.generation = self.tab_bar_status.generation.wrapping_add(1);
        self.tab_bar_status.datetimes.clear();
        self.tab_bar_status.commands.clear();
        self.state.tab_bar_right.clear();
        self.state.tab_bar_right_separator = sanitize_separator(separator);
        let now = Instant::now();
        for entry in entries.iter().take(MAX_TAB_BAR_RIGHT_ENTRIES) {
            self.add_tab_bar_status_entry(entry, now);
        }
        self.tab_bar_status.next_datetime_refresh =
            (!self.tab_bar_status.datetimes.is_empty()).then_some(now + DATETIME_REFRESH_INTERVAL);
    }

    /// Projects one qualified preference into its producer and ordered segment.
    fn add_tab_bar_status_entry(&mut self, entry: &TabBarRightEntryConfig, now: Instant) {
        let segment_index = self.state.tab_bar_right.len();
        let segment = match entry {
            TabBarRightEntryConfig::Zoom {} => TabBarStatusSegment::Zoom,
            TabBarRightEntryConfig::Hostname {} => TabBarStatusSegment::Text(sanitize_status_text(
                crate::platform::hostname().as_deref().unwrap_or_default(),
            )),
            TabBarRightEntryConfig::Text { text } => {
                TabBarStatusSegment::Text(sanitize_literal_text(text))
            }
            TabBarRightEntryConfig::Datetime { format } => {
                let Ok(format) = parse_tab_bar_datetime_format(format) else {
                    return;
                };
                let value = format_local_datetime(&format);
                self.tab_bar_status.datetimes.push(TabBarDatetimeRuntime {
                    segment_index,
                    format,
                });
                TabBarStatusSegment::Text(value)
            }
            TabBarRightEntryConfig::Command {
                command,
                interval_seconds,
                timeout_seconds,
            } => {
                if !cfg!(unix)
                    || command.trim().is_empty()
                    || !(1..=MAX_TAB_BAR_COMMAND_INTERVAL_SECONDS).contains(interval_seconds)
                    || !(1..=MAX_TAB_BAR_COMMAND_TIMEOUT_SECONDS).contains(timeout_seconds)
                {
                    return;
                }
                self.tab_bar_status.commands.push(TabBarCommandRuntime {
                    segment_index,
                    command: command.clone(),
                    interval: Duration::from_secs(*interval_seconds),
                    timeout: Duration::from_secs(*timeout_seconds),
                    next_run_at: now,
                    task: None,
                });
                TabBarStatusSegment::Text(None)
            }
        };
        self.state.tab_bar_right.push(segment);
    }

    /// Returns the configuration identity attached to current status command events.
    #[must_use]
    pub const fn tab_bar_status_generation(&self) -> u64 {
        self.tab_bar_status.generation
    }

    /// Refreshes due local calendars and starts idle commands from their launch time.
    pub fn handle_tab_bar_status_tasks(&mut self, now: Instant) -> bool {
        let changed = self.refresh_tab_bar_datetimes(now);
        if !self
            .tab_bar_status
            .commands
            .iter()
            .any(|runtime| runtime.task.is_none() && now >= runtime.next_run_at)
        {
            return changed;
        }
        let (environment, cwd) = self.custom_command_env();
        for runtime in &mut self.tab_bar_status.commands {
            if runtime.task.is_some() || now < runtime.next_run_at {
                continue;
            }
            runtime.next_run_at = now.checked_add(runtime.interval).unwrap_or(now);
            runtime.task = Some(spawn_status_command(
                self.launch.events.clone(),
                StatusCommandRequest {
                    generation: self.tab_bar_status.generation,
                    segment_index: runtime.segment_index,
                    command: runtime.command.clone(),
                    timeout: runtime.timeout,
                    environment: environment.clone(),
                    cwd: cwd.clone(),
                },
            ));
        }
        changed
    }

    /// Refreshes each calendar segment at most once for a supplied deadline.
    fn refresh_tab_bar_datetimes(&mut self, now: Instant) -> bool {
        if self
            .tab_bar_status
            .next_datetime_refresh
            .is_none_or(|deadline| now < deadline)
        {
            return false;
        }
        let mut changed = false;
        for runtime in &self.tab_bar_status.datetimes {
            let value = format_local_datetime(&runtime.format);
            if let Some(TabBarStatusSegment::Text(current)) =
                self.state.tab_bar_right.get_mut(runtime.segment_index)
            {
                changed |= *current != value;
                *current = value;
            }
        }
        self.tab_bar_status.next_datetime_refresh = Some(now + DATETIME_REFRESH_INTERVAL);
        changed
    }

    /// Selects one next status deadline, excluding already-running commands.
    #[must_use]
    pub fn next_tab_bar_status_deadline(&self) -> Option<Instant> {
        self.tab_bar_status
            .commands
            .iter()
            .filter(|runtime| runtime.task.is_none())
            .map(|runtime| runtime.next_run_at)
            .chain(self.tab_bar_status.next_datetime_refresh)
            .min()
    }

    /// Accepts a result only for an extant command in the current configuration.
    pub(super) fn handle_tab_bar_command_finished(
        &mut self,
        generation: u64,
        segment_index: usize,
        result: Result<Option<String>, String>,
    ) -> bool {
        if generation != self.tab_bar_status.generation {
            return false;
        }
        let Some(runtime) = self
            .tab_bar_status
            .commands
            .iter_mut()
            .find(|runtime| runtime.segment_index == segment_index)
        else {
            return false;
        };
        runtime.task = None;
        let output = result.unwrap_or_else(|error| {
            tracing::warn!(command = %runtime.command, error, "tab bar status command failed");
            None
        });
        let Some(TabBarStatusSegment::Text(current)) =
            self.state.tab_bar_right.get_mut(segment_index)
        else {
            return false;
        };
        let changed = *current != output;
        *current = output;
        changed
    }
}

/// Formats the local wall clock and applies dynamic status sanitation.
fn format_local_datetime(format: &time::format_description::OwnedFormatItem) -> Option<String> {
    crate::platform::local_datetime()?
        .format(format)
        .ok()
        .and_then(|value| sanitize_status_text(&value))
}
