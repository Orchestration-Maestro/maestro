//! Passive OSC title, progress, directory and diagnostic evidence.

mod collector;
mod colors;
pub(super) use colors::{
    DefaultColorEvent, DefaultColorEventTracker, DefaultColorQuery, DefaultColorTrackedEvent,
};
mod cwd;
mod process;
use collector::OscStreamCollector;
pub(super) use cwd::parse_reported_cwd;
pub(super) use process::{
    contains_scrollback_clear_sequence, maybe_filter_primary_screen_scrollback_clear,
    should_restore_host_terminal_theme,
};

/// Maximum retained title and progress length in Unicode scalar values.
const AGENT_OSC_MAX_CHARS: usize = 256;

/// Latest display title and independently clearable agent evidence.
#[derive(Debug, Default)]
pub(super) struct AgentOscStateTracker {
    /// Incremental OSC framing.
    collector: OscStreamCollector,
    /// Latest detection title, cleared on agent changes.
    latest_title: Option<String>,
    /// Display title, retained on agent changes.
    terminal_title: Option<String>,
    /// Latest detection progress.
    latest_progress: Option<String>,
}

impl AgentOscStateTracker {
    /// Retain title and progress; report whether the display title changed.
    pub(super) fn observe(&mut self, bytes: &[u8]) -> bool {
        let mut changed = false;
        self.collector.observe(bytes, |body, _| {
            let Some((command, payload)) = split_body(body) else {
                return;
            };
            match command {
                b"0" | b"2" => {
                    let title = sanitize(payload, AGENT_OSC_MAX_CHARS);
                    let title = (!title.is_empty()).then_some(title);
                    changed |= self.terminal_title != title;
                    self.terminal_title.clone_from(&title);
                    self.latest_title = title;
                }
                b"9" => self.latest_progress = Some(sanitize(payload, AGENT_OSC_MAX_CHARS)),
                _ => {}
            }
        });
        changed
    }

    /// Latest nonempty display title.
    pub(super) fn terminal_title(&self) -> Option<&str> {
        self.terminal_title.as_deref()
    }

    /// Restore a display title without reviving agent evidence.
    pub(super) fn seed_terminal_title(&mut self, title: Option<String>) {
        self.terminal_title = title;
    }

    /// Latest agent title, or an empty string after clearing.
    pub(super) fn latest_title(&self) -> &str {
        self.latest_title.as_deref().unwrap_or("")
    }

    /// Latest agent progress, or an empty string before observation.
    pub(super) fn latest_progress(&self) -> &str {
        self.latest_progress.as_deref().unwrap_or("")
    }

    /// Clear agent evidence while preserving framing and the display title.
    pub(super) fn clear_retained(&mut self) {
        self.latest_title = None;
        self.latest_progress = None;
    }
}

/// Optional, bounded diagnostic OSC capture.
#[derive(Debug)]
pub(super) struct OscDebugTracker {
    /// Whether diagnostic capture is active.
    enabled: bool,
    /// Incremental framing independent of observation chunks.
    collector: OscStreamCollector,
    /// Completed evidence awaiting logging.
    pending: Vec<OscDebugEvent>,
}

/// One sanitized diagnostic event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct OscDebugEvent {
    /// OSC command number.
    pub(super) command: String,
    /// Sanitized bounded payload.
    pub(super) payload: String,
}

impl OscDebugTracker {
    /// Configure capture without changing process-wide environment.
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            collector: OscStreamCollector::default(),
            pending: Vec::new(),
        }
    }

    /// Retain selected commands only when evidence capture is enabled.
    pub(super) fn observe(&mut self, bytes: &[u8]) {
        if !self.enabled {
            return;
        }
        self.collector.observe(bytes, |body, _| {
            let Some((command, payload)) = split_body(body) else {
                return;
            };
            if !matches!(command, b"0" | b"2" | b"9" | b"21337") {
                return;
            }
            let mut sanitized = sanitize(payload, 512);
            if String::from_utf8_lossy(payload).chars().count() > 512 {
                sanitized.push_str("...");
            }
            self.pending.push(OscDebugEvent {
                command: String::from_utf8_lossy(command).into_owned(),
                payload: sanitized,
            });
        });
    }

    /// Take completed evidence without discarding incomplete framing.
    pub(super) fn drain_pending(&mut self) -> Vec<OscDebugEvent> {
        std::mem::take(&mut self.pending)
    }
}

impl Default for OscDebugTracker {
    fn default() -> Self {
        Self::new(
            std::env::var("MAESTRO_DEBUG_OSC_EVIDENCE").is_ok_and(|value| {
                crate::variables::flag_enabled(&value, crate::variables::FlagPolicy::Normalized)
            }),
        )
    }
}

/// Separate the numeric OSC command from its payload.
fn split_body(body: &[u8]) -> Option<(&[u8], &[u8])> {
    let separator = body.iter().position(|byte| *byte == b';')?;
    Some((&body[..separator], &body[separator + 1..]))
}

/// Remove controls before bounding retained Unicode scalar values.
fn sanitize(payload: &[u8], limit: usize) -> String {
    String::from_utf8_lossy(payload)
        .chars()
        .filter(|ch| !ch.is_control())
        .take(limit)
        .collect()
}

#[cfg(test)]
mod tests;
