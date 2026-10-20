//! Screen scan and publication policy for pane detection.

use maestro_detection::agents::{Agent, AgentDetection, AgentState, AgentVisibility};
use std::time::{Duration, Instant};

/// Observations used to skip unchanged idle screens.
#[derive(Debug, Clone, Copy)]
pub struct DetectionScreenReadInput {
    /// Previously published state.
    pub state: AgentState,
    /// Retained foreground identity.
    pub agent: Option<Agent>,
    /// Whether an idle transition awaits confirmation.
    pub pending_idle_active: bool,
    /// Whether the foreground identity changed.
    pub agent_changed: bool,
    /// Whether the foreground process exited.
    pub process_exited: bool,
    /// Latest terminal content revision.
    pub current_detection_content_seq: Option<u64>,
    /// Content revision of the last screen scan.
    pub last_screen_scan_detection_content_seq: Option<u64>,
}

/// Whether a detector needs fresh terminal text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionScreenReadDecision {
    /// Read the live bottom buffer.
    Read,
    /// Reuse the unchanged idle observation.
    Skip,
}

/// Skips only an identified, unchanged idle screen without pending transitions.
#[must_use]
pub fn decide_detection_screen_read(
    input: DetectionScreenReadInput,
) -> DetectionScreenReadDecision {
    if input.state == AgentState::Idle
        && input.agent.is_some()
        && !input.pending_idle_active
        && !input.agent_changed
        && !input.process_exited
        && input.current_detection_content_seq.is_some()
        && input.current_detection_content_seq == input.last_screen_scan_detection_content_seq
    {
        DetectionScreenReadDecision::Skip
    } else {
        DetectionScreenReadDecision::Read
    }
}

/// Interval between plain-idle confirmations.
pub const AGENT_PENDING_IDLE_RECHECK: Duration = Duration::from_millis(100);
/// Maximum duration of a plain-idle confirmation.
pub const AGENT_PENDING_IDLE_CAP: Duration = Duration::from_millis(700);

/// Published state and its recognized screen indicators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionPublishState {
    /// Published execution state.
    pub state: AgentState,
    /// Published live-screen evidence.
    pub visible: AgentVisibility,
}

/// Observations for one publication transition.
#[derive(Debug, Clone, Copy)]
pub struct DetectionTransitionInput {
    /// Last published observation.
    pub previous_publish: DetectionPublishState,
    /// Proposed screen observation.
    pub next_publish: DetectionPublishState,
    /// Whether process identity changed.
    pub agent_changed: bool,
    /// Whether the retained process exited.
    pub process_exited: bool,
    /// Whether unchanged blocker evidence needs refreshing.
    pub stable_refresh_due: bool,
    /// Time of this observation.
    pub now: Instant,
}

/// Result of applying confirmation and change policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionTransitionDecision {
    /// Keep the previous published observation.
    NoPublish,
    /// Publish the proposed observation.
    PublishNext,
}

/// Retained confirmation for a working-to-plain-idle transition.
#[derive(Debug, Default)]
pub struct PendingIdleConfirmation {
    /// First plain-idle observation.
    started_at: Option<Instant>,
    /// Subsequent consecutive plain-idle observations.
    confirmations: u8,
}

impl PendingIdleConfirmation {
    /// Whether plain idle still awaits confirmation.
    #[must_use]
    pub fn active(&self) -> bool {
        self.started_at.is_some()
    }

    /// Discard a pending plain-idle observation.
    pub fn clear(&mut self) {
        self.started_at = None;
        self.confirmations = 0;
    }

    /// Holds plain idle for three further observations or the confirmation cap.
    fn should_hold_working_to_idle(&mut self, input: DetectionTransitionInput) -> bool {
        let next = input.next_publish;
        if input.previous_publish.state != AgentState::Working
            || next.state != AgentState::Idle
            || next.visible.visible_idle
            || next.visible.visible_blocker
            || input.agent_changed
            || input.process_exited
        {
            self.clear();
            return false;
        }
        let Some(started) = self.started_at else {
            self.started_at = Some(input.now);
            self.confirmations = 0;
            return true;
        };
        self.confirmations = self.confirmations.saturating_add(1);
        if input.now.duration_since(started) >= AGENT_PENDING_IDLE_CAP || self.confirmations >= 3 {
            self.clear();
            return false;
        }
        true
    }
}

/// Applies pending-idle confirmation before deciding whether to publish changes.
#[must_use]
pub fn decide_detection_transition(
    input: DetectionTransitionInput,
    pending_idle: &mut PendingIdleConfirmation,
) -> DetectionTransitionDecision {
    if pending_idle.should_hold_working_to_idle(input) {
        return DetectionTransitionDecision::NoPublish;
    }
    let blocker_refresh = input.stable_refresh_due
        && input.previous_publish.visible.visible_blocker
        && input.next_publish.visible.visible_blocker;
    if input.previous_publish != input.next_publish
        || input.agent_changed
        || input.process_exited
        || blocker_refresh
    {
        DetectionTransitionDecision::PublishNext
    } else {
        DetectionTransitionDecision::NoPublish
    }
}

/// Interval for refreshing unchanged visible blocker evidence.
pub const STABLE_VISIBLE_SIGNAL_REFRESH: Duration = Duration::from_millis(800);

/// Screen observation and the previously published state.
#[derive(Debug, Clone, Copy)]
pub struct ScreenDetectionPublishInput {
    /// Last published state and screen evidence.
    pub previous: DetectionPublishState,
    /// Time of the last published blocker or working evidence.
    pub last_visible_signal_refresh: Option<Instant>,
    /// Detector result for the live bottom buffer.
    pub screen_detection: AgentDetection,
    /// Whether the foreground process exited.
    pub process_exited: bool,
    /// Whether process identity changed.
    pub agent_changed: bool,
    /// Time of this observation.
    pub now: Instant,
}

/// Screen publication selected after transition confirmation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionPublishDecision {
    /// Preserve the previous publication.
    NoPublish,
    /// Publish a normalized observation.
    Publish {
        /// State and screen indicators consistent with it.
        next: DetectionPublishState,
        /// Whether this observation reports process completion.
        process_exited: bool,
    },
}

/// Normalizes screen evidence and applies idle-confirmation and refresh policy.
#[must_use]
pub fn decide_screen_detection_publish(
    input: ScreenDetectionPublishInput,
    pending_idle: &mut PendingIdleConfirmation,
) -> DetectionPublishDecision {
    let detection = input.screen_detection;
    let state = crate::terminal::state::stabilize_agent_detection(detection);
    let next = DetectionPublishState {
        state,
        visible: AgentVisibility {
            visible_idle: detection.visible.visible_idle && state == AgentState::Idle,
            visible_blocker: detection.visible.visible_blocker && state == AgentState::Blocked,
            visible_working: detection.visible.visible_working && state == AgentState::Working,
        },
    };
    let stable_refresh_due = input.previous.visible.visible_blocker
        && next.visible.visible_blocker
        && input
            .last_visible_signal_refresh
            .is_none_or(|last| input.now.duration_since(last) >= STABLE_VISIBLE_SIGNAL_REFRESH);
    match decide_detection_transition(
        DetectionTransitionInput {
            previous_publish: input.previous,
            next_publish: next,
            agent_changed: input.agent_changed,
            process_exited: input.process_exited,
            stable_refresh_due,
            now: input.now,
        },
        pending_idle,
    ) {
        DetectionTransitionDecision::NoPublish => DetectionPublishDecision::NoPublish,
        DetectionTransitionDecision::PublishNext => DetectionPublishDecision::Publish {
            next,
            process_exited: input.process_exited,
        },
    }
}

/// Invalidates detection scan reuse after any nonempty raw PTY read.
pub fn observe_detection_content_change(
    bytes: &[u8],
    detection_content_seq: &std::sync::atomic::AtomicU64,
) {
    if !bytes.is_empty() {
        mark_detection_content_changed(detection_content_seq);
    }
}

/// Invalidates detection scan reuse after a local terminal mutation.
pub fn mark_detection_content_changed(detection_content_seq: &std::sync::atomic::AtomicU64) {
    detection_content_seq.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

#[cfg(test)]
mod tests;
