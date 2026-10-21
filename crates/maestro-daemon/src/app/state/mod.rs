//! Mutable daemon data with one topology allocator.

use crate::{settings::model::Config, terminal::state::TerminalState};
use maestro_detection::{
    manifest::{AgentManifestSummary, ManifestCache},
    manifest_update::ManifestUpdateStatus,
};
use maestro_workspace::terminal_id::TerminalId;
use maestro_workspace::{
    layout::{PaneId, PaneInfo},
    workspace::WorkspaceTopology,
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

mod notification;
pub use notification::{
    AgentNotificationDelivery, PendingAgentNotification, ToastKind, ToastNotification, ToastTarget,
};
mod selection;
pub use selection::{
    CopyModeSelection, CopyModeState, Mode, SelectionAutoscroll, SelectionAutoscrollDirection,
};

/// Logical pane focus that survives workspace reordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneFocusTarget {
    /// Stable workspace identity.
    pub workspace_id: String,
    /// Pane identity allocated by the topology.
    pub pane_id: PaneId,
}

/// Extension provenance for a registered pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPaneRecord {
    /// Owning extension identity.
    pub plugin_id: String,
    /// Registered pane entrypoint.
    pub entrypoint: String,
}

/// Mutable workspace data retained independently of attached clients.
pub struct DaemonState<C> {
    /// Sole owner of workspace topology and public identity allocation.
    pub topology: WorkspaceTopology<C>,
    /// Retained terminal metadata keyed by the attached terminal identity.
    pub terminals: HashMap<TerminalId, TerminalState>,
    /// Compiled detection snapshot shared with pane runtimes.
    pub manifests: Arc<RwLock<ManifestCache>>,
    /// Active detection sources and versions from the shared snapshot.
    pub agent_manifest_summaries: Vec<AgentManifestSummary>,
    /// Diagnostics from the latest detection catalogue update.
    pub agent_manifest_update_status: ManifestUpdateStatus,
    /// Registered pane provenance removed with its owning pane.
    pub plugin_panes: HashMap<PaneId, PluginPaneRecord>,
    /// Detached metadata identities awaiting runtime shutdown by the loop.
    pub terminal_runtime_shutdowns: Vec<TerminalId>,
    /// Active workspace display position, absent when no workspace is active.
    pub active: Option<usize>,
    /// Workspace selection position for navigation.
    pub selected: usize,
    /// Structural changes awaiting snapshot scheduling.
    pub session_dirty: bool,
    /// Background notification preferences applied independently of sound.
    pub toast_config: crate::settings::model::ToastConfig,
    /// Per-agent sound admission preferences.
    pub sound: crate::settings::sound::SoundConfig,
    /// Last known outer terminal focus, absent before the first report.
    pub outer_terminal_focus: Option<bool>,
    /// Most recent workspace notification.
    pub toast: Option<ToastNotification>,
    /// Available application version reported by the update producer.
    pub update_available: Option<String>,
    /// Install command supplied by the update producer.
    pub update_install_command: String,
    /// Whether release notes are available for the reported version.
    pub latest_release_notes_available: bool,
    /// Whether automatic update prompting has been dismissed.
    pub update_dismissed: bool,
    /// Delayed transitions revalidated before delivery.
    pub pending_agent_notifications: HashMap<PaneId, PendingAgentNotification>,
    /// Sequence shared by effective agent state changes.
    pub next_agent_state_change_seq: u64,
    /// Whether implicit parent-group closes require confirmation.
    pub confirm_close: maestro_protocol::shared::Flag,
    /// Current interaction context.
    pub mode: Mode,
    /// Keyboard copy pane retained across focus navigation.
    pub copy_mode: Option<CopyModeState>,
    /// Current terminal text selection.
    pub selection: Option<crate::selection::Selection>,
    /// Pending drag geometry cleared with its selection.
    pub selection_autoscroll: Option<SelectionAutoscroll>,
    /// Previous logical focus for last-pane navigation.
    pub previous_pane_focus: Option<PaneFocusTarget>,
    /// Historical numeric spellings mapped to their retained pane identity.
    pub pane_id_aliases: HashMap<u32, PaneId>,
    /// Historical public spellings mapped to their retained pane identity.
    pub public_pane_id_aliases: HashMap<String, PaneId>,
    /// Virtual terminal dimensions in columns and rows before a view attaches.
    pub headless_size: (u16, u16),
    /// Most recently supplied pane geometry, in view order.
    pub pane_infos: Vec<PaneInfo>,
}

impl<C: FnMut() -> u128> DaemonState<C> {
    /// Creates empty daemon state using accepted settings and an identity clock.
    #[must_use]
    pub fn new(config: &Config, clock: C, manifests: Arc<RwLock<ManifestCache>>) -> Self {
        Self {
            topology: WorkspaceTopology::new(clock),
            terminals: HashMap::new(),
            manifests,
            agent_manifest_summaries: Vec::new(),
            agent_manifest_update_status: ManifestUpdateStatus::default(),
            plugin_panes: HashMap::new(),
            terminal_runtime_shutdowns: Vec::new(),
            active: None,
            selected: 0,
            session_dirty: false,
            previous_pane_focus: None,
            toast_config: config.ui.toast.clone(),
            sound: config.ui.sound.clone(),
            outer_terminal_focus: None,
            toast: None,
            update_available: None,
            update_install_command: "maestro update".into(),
            latest_release_notes_available: false,
            update_dismissed: false,
            pending_agent_notifications: HashMap::new(),
            next_agent_state_change_seq: 0,
            confirm_close: config.ui.confirm_close,
            mode: Mode::Navigate,
            copy_mode: None,
            selection: None,
            selection_autoscroll: None,
            pane_id_aliases: HashMap::new(),
            public_pane_id_aliases: HashMap::new(),
            headless_size: config.headless_size(),
            pane_infos: Vec::new(),
        }
    }

    /// Estimates rows and columns from the first pane, or unattached dimensions.
    #[must_use]
    pub fn estimate_pane_size(&self) -> (u16, u16) {
        self.pane_infos
            .first()
            .map_or((self.headless_size.1, self.headless_size.0), |pane| {
                (pane.rect.height, pane.rect.width)
            })
    }
}
