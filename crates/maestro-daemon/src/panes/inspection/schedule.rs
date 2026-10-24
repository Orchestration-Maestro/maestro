//! Foreground process probe scheduling from observed job groups.

use std::time::Duration;

use maestro_detection::agents::Agent;

/// Identified agents receive a periodic process check.
pub const PROCESS_RECHECK_IDENTIFIED: Duration = Duration::from_secs(5);
/// Unknown panes without a foreground group receive a safety check.
pub const PROCESS_RECHECK_MISSING_FOREGROUND_GROUP: Duration = Duration::from_secs(30);
/// Maximum duration of a same-group acquisition burst.
pub const PROCESS_ACQUISITION_WINDOW: Duration = Duration::from_secs(8);
/// Initial fast portion of an acquisition burst.
pub const PROCESS_ACQUISITION_FAST_WINDOW: Duration = Duration::from_millis(1500);
/// Process check interval during fast acquisition.
pub const PROCESS_ACQUISITION_FAST_RECHECK: Duration = Duration::from_millis(500);
/// Process check interval during slow acquisition.
pub const PROCESS_ACQUISITION_SLOW_RECHECK: Duration = Duration::from_secs(2);
/// Quiet interval required before restarting an expired burst.
pub const PROCESS_ACQUISITION_IDLE_RESET: Duration = Duration::from_secs(2);

/// Observations used to decide whether a full process scan is due.
#[derive(Debug, Clone, Copy)]
pub struct ProcessProbeInput {
    /// Currently retained identity.
    pub current_agent: Option<Agent>,
    /// Identity whose lifecycle release is pending.
    pub suppressed_agent: Option<Agent>,
    /// Kernel-observed foreground group.
    pub foreground_pgid: Option<u32>,
    /// Previously tracked kernel-observed group.
    pub last_foreground_pgid: Option<u32>,
    /// Whether a full scan has already completed.
    pub has_process_probe: bool,
    /// Time since the current acquisition burst began.
    pub acquisition_age: Option<Duration>,
    /// Whether process exit still needs to be published.
    pub pending_foreground_shell_clear: bool,
    /// Whether restored identity needs confirmation.
    pub pending_restore_probe: bool,
    /// Time since the last full process scan.
    pub elapsed_since_process_check: Duration,
}

/// Returns whether the observed group changed, including disappearance.
#[must_use]
pub fn foreground_group_changed(current: Option<u32>, previous: Option<u32>) -> bool {
    current != previous
}

/// Tracks only kernel observations, never a group inferred by a full scan.
#[must_use]
pub fn process_group_for_change_tracking(
    observed: Option<u32>,
    probed: Option<u32>,
) -> Option<u32> {
    observed?;
    probed.or(observed)
}

/// Skips routine stable-group scans when lifecycle reports own the identity.
#[must_use]
pub fn should_skip_process_probe_for_lifecycle_authority(
    active: bool,
    input: ProcessProbeInput,
) -> bool {
    active
        && input.foreground_pgid.is_some()
        && !input.pending_foreground_shell_clear
        && input.suppressed_agent.is_none()
        && input.has_process_probe
        && !foreground_group_changed(input.foreground_pgid, input.last_foreground_pgid)
}

/// Refreshes the foreground-group observation for transitions or periodic checks.
#[must_use]
pub fn should_observe_foreground_process_group(
    input: ProcessProbeInput,
    observation: ForegroundObservation,
) -> bool {
    !input.has_process_probe
        || input.current_agent.is_none()
        || input.suppressed_agent.is_some()
        || input.pending_foreground_shell_clear
        || input.pending_restore_probe
        || observation.content_changed
        || (observation.lifecycle_authority && observation.elapsed >= PROCESS_RECHECK_IDENTIFIED)
        || (!observation.lifecycle_authority
            && input.elapsed_since_process_check >= PROCESS_RECHECK_IDENTIFIED)
}

/// Screen and lifecycle observations used to schedule foreground-group reads.
#[derive(Debug, Clone, Copy)]
pub struct ForegroundObservation {
    /// Whether integration reports own lifecycle transitions.
    pub lifecycle_authority: bool,
    /// Whether terminal content changed.
    pub content_changed: bool,
    /// Time since the last foreground-group observation.
    pub elapsed: Duration,
}

/// Returns whether a full process scan is due.
#[must_use]
pub fn should_probe_foreground_job(input: ProcessProbeInput) -> bool {
    if input.pending_foreground_shell_clear || input.pending_restore_probe {
        return true;
    }
    let changed = foreground_group_changed(input.foreground_pgid, input.last_foreground_pgid);
    if input.suppressed_agent.is_some() {
        return !input.has_process_probe || changed;
    }
    if let Some(age) = input.acquisition_age {
        let interval = if age <= PROCESS_ACQUISITION_FAST_WINDOW {
            PROCESS_ACQUISITION_FAST_RECHECK
        } else {
            PROCESS_ACQUISITION_SLOW_RECHECK
        };
        if age <= PROCESS_ACQUISITION_WINDOW && input.elapsed_since_process_check >= interval {
            return true;
        }
    }
    if input.current_agent.is_none() {
        return !input.has_process_probe
            || changed
            || (input.foreground_pgid.is_none()
                && input.elapsed_since_process_check >= PROCESS_RECHECK_MISSING_FOREGROUND_GROUP);
    }
    changed || input.elapsed_since_process_check >= PROCESS_RECHECK_IDENTIFIED
}

/// Bounded retry window for terminal output from unidentified processes.
#[derive(Debug, Default)]
pub struct AcquisitionWindow {
    /// Beginning of the current retry burst.
    pub started_at: Option<std::time::Instant>,
    /// Most recent content change during the burst.
    pub last_content_change_at: Option<std::time::Instant>,
}

impl AcquisitionWindow {
    /// Starts or expires a burst only when no identity or group-change signal exists.
    pub fn sync(
        &mut self,
        input: ProcessProbeInput,
        content_changed: bool,
        now: std::time::Instant,
    ) {
        if input.current_agent.is_some()
            || input.suppressed_agent.is_some()
            || foreground_group_changed(input.foreground_pgid, input.last_foreground_pgid)
        {
            return;
        }
        if content_changed {
            let quiet = self
                .last_content_change_at
                .is_none_or(|last| now.duration_since(last) >= PROCESS_ACQUISITION_IDLE_RESET);
            let should_start = self.started_at.is_none_or(|started| {
                now.duration_since(started) > PROCESS_ACQUISITION_WINDOW && quiet
            });
            if should_start {
                self.started_at = Some(now);
            }
            self.last_content_change_at = Some(now);
        } else if let (Some(started), Some(last)) = (self.started_at, self.last_content_change_at)
            && now.duration_since(started) > PROCESS_ACQUISITION_WINDOW
            && now.duration_since(last) >= PROCESS_ACQUISITION_IDLE_RESET
        {
            self.started_at = None;
            self.last_content_change_at = None;
        }
    }
}

#[cfg(test)]
mod tests;
