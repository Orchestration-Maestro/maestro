//! Background pane events delivered to the workspace loop.

use std::time::Instant;

use maestro_detection::agents::{Agent, AgentState};
use maestro_workspace::layout::PaneId;
use tokio::sync::mpsc;

/// An observation from a background pane task.
#[derive(Debug)]
pub enum AppEvent {
    /// A foreground agent was identified before screen state confirmation.
    AgentProcessDetected {
        /// Pane that owns the process.
        pane_id: PaneId,
        /// Recognized agent identity.
        agent: Agent,
        /// Time of the process observation.
        observed_at: Instant,
    },
    /// Fallback screen detection changed a pane's state.
    StateChanged {
        /// Pane whose state changed.
        pane_id: PaneId,
        /// Retained foreground identity.
        agent: Option<Agent>,
        /// Normalized screen state.
        state: AgentState,
        /// Whether the screen contains a visible blocker.
        visible_blocker: bool,
        /// Whether the screen contains visible working evidence.
        visible_working: bool,
        /// Whether this observation reports process completion.
        process_exited: bool,
        /// Time of the screen observation.
        observed_at: Instant,
    },
}

impl AppEvent {
    /// Waits for queue space without dropping correctness-critical transitions.
    pub async fn publish(self, sender: &mpsc::Sender<Self>) {
        if let Err(error) = sender.send(self).await {
            tracing::warn!(%error, "failed to deliver pane event");
        }
    }
}
