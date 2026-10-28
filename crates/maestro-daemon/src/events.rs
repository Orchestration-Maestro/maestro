//! Background pane events delivered to the workspace loop.

use std::time::Instant;

use crate::terminal::state::{AgentMetadataReport, SessionStartReport};
use maestro_detection::agents::{Agent, AgentState};
use maestro_workspace::layout::PaneId;
use tokio::sync::mpsc;

/// An observation from a background pane task.
#[derive(Debug)]
pub enum AppEvent {
    /// A retained status command completed against one configuration generation.
    TabBarCommandFinished {
        /// Configuration generation that launched the command.
        generation: u64,
        /// Ordered segment receiving this output.
        segment_index: usize,
        /// Last sanitized output line, or the command failure.
        result: Result<Option<String>, String>,
    },
    /// The pane's child or imported terminal reader has exited.
    PaneDied {
        /// Pane whose runtime ended.
        pane_id: PaneId,
    },
    /// Bells aggregated from one terminal read.
    TerminalBell {
        /// Pane requesting attention.
        pane_id: PaneId,
        /// Bell count from this read.
        count: u16,
    },
    /// A newly admitted absolute working directory report.
    TerminalCwdReported {
        /// Pane reporting its working directory.
        pane_id: PaneId,
        /// Accepted directory spelling.
        cwd: std::path::PathBuf,
    },
    /// Clipboard content emitted by the child terminal.
    ClipboardWrite {
        /// Decoded clipboard bytes.
        content: Vec<u8>,
    },
    /// A foreground agent was identified before screen state confirmation.
    AgentProcessDetected {
        /// Pane that owns the process.
        pane_id: PaneId,
        /// Recognized agent identity.
        agent: Agent,
        /// Time of the process observation.
        observed_at: Instant,
    },
    /// Hook-authoritative state reported by an integration.
    HookStateReported {
        /// Pane receiving the report.
        pane_id: PaneId,
        /// Integration provenance.
        source: String,
        /// External catalogue label or custom identity.
        agent_label: String,
        /// Reported execution state.
        state: AgentState,
        /// Optional status detail.
        message: Option<String>,
        /// Source-local sequence.
        seq: Option<u64>,
        /// Validated session identity.
        session_ref: Option<crate::agent_resume::AgentSessionRef>,
    },
    /// Agent session identity was reported without state authority.
    AgentSessionReported {
        /// Pane receiving the report.
        pane_id: PaneId,
        /// Supplied identity, freshness and lifecycle-start provenance.
        report: SessionStartReport,
    },
    /// Display-only agent metadata was reported for a pane.
    HookMetadataReported {
        /// Pane receiving the report.
        pane_id: PaneId,
        /// Canonical presentation update with its clear, guard and lifetime controls.
        report: AgentMetadataReport,
    },
    /// Integration authority was explicitly cleared for a pane.
    HookAuthorityCleared {
        /// Pane receiving the clear.
        pane_id: PaneId,
        /// Integration provenance whose authority is cleared, or any when absent.
        source: Option<String>,
        /// Source-local sequence.
        seq: Option<u64>,
    },
    /// Integration ownership released back to the shell.
    HookAgentReleased {
        /// Pane receiving the release.
        pane_id: PaneId,
        /// Integration provenance.
        source: String,
        /// External catalogue label or custom identity.
        agent_label: String,
        /// Recognized catalogue identity, when available.
        known_agent: Option<Agent>,
        /// Source-local sequence.
        seq: Option<u64>,
    },
    /// An application version is available through the active installation manager.
    UpdateReady {
        /// Available version spelling.
        version: String,
        /// Installation command selected by the producer.
        install_command: String,
    },
    /// Detection catalogue publication has completed against the shared cache.
    AgentDetectionManifestsUpdated {
        /// Successfully published agent versions.
        updated: Vec<maestro_detection::manifest_update::ManifestUpdateCommit>,
        /// Latest catalogue diagnostics, including checks with no updates.
        status: maestro_detection::manifest_update::ManifestUpdateStatus,
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
