//! Mutable daemon data with one topology allocator.

use crate::{settings::model::Config, terminal::model::TerminalState};
use maestro_detection::{
    definitions::{AgentManifestSummary, ManifestCache},
    updates::ManifestUpdateStatus,
};
use maestro_workspace::terminal_id::TerminalId;
use maestro_workspace::{
    spaces::WorkspaceTopology,
    tiling::{PaneId, PaneInfo, Rect},
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// Agent panel ordering uses the same vocabulary as the accepted preferences.
pub use crate::settings::model::AgentPanelSortConfig as AgentPanelSort;

mod notification;
pub use notification::{AgentNotificationDelivery, PendingAgentNotification};
pub use notification::{CopyFeedback, ToastKind, ToastNotification, ToastTarget};
mod theme;
pub use theme::ThemeRuntimeConfig;
mod selection;
pub use selection::{
    CopyModeSelection, CopyModeState, Mode, SelectionAutoscroll, SelectionAutoscrollDirection,
};

/// Logical pane focus that survives workspace reordering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneFocusTarget {
    /// Stable workspace identity.
    pub(crate) workspace_id: String,
    /// Pane identity allocated by the topology.
    pub(crate) pane_id: PaneId,
}

/// Workspace-action provenance for a registered pane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActionPaneRecord {
    /// Owning workspace action identity.
    pub(crate) action_id: String,
    /// Registered pane entrypoint.
    pub(crate) entrypoint: String,
}

/// One retained right-side tab-bar segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabBarStatusSegment {
    /// The current tab's zoom indicator, resolved by its renderer.
    Zoom,
    /// Sanitized text, absent when the producer has no visible output.
    Text(Option<String>),
}

/// Source controlling whether a settings reload replaces the live sidebar width.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SidebarWidthSource {
    /// The configured default still owns the live width.
    #[default]
    ConfigDefault,
    /// A restored session supplied the live width.
    Persisted,
    /// A user resize supplied the live width.
    Manual,
}

/// Mutable workspace data retained independently of attached clients.
pub struct DaemonState {
    /// Whether the shared daemon loop should finish its owned shutdown.
    pub(crate) should_quit: maestro_protocol::shared::Flag,
    /// Live expanded sidebar width, independently resizable from its configured default.
    pub(crate) sidebar_width: u16,
    /// Persisted fraction separating sidebar sections.
    pub(crate) sidebar_section_split: f32,
    /// Persisted collapsed repository group identities.
    pub(crate) collapsed_space_keys: std::collections::HashSet<String>,
    /// Owner of the live sidebar width.
    pub(crate) sidebar_width_source: SidebarWidthSource,
    /// Current agent-panel scroll position, reset when row composition reloads.
    pub(crate) agent_panel_scroll: usize,
    /// Compact diagnostic for the most recently loaded settings document.
    pub(crate) config_diagnostic: Option<String>,
    /// Whether connected clients must reacquire accepted interface preferences.
    pub(crate) request_client_config_reload: maestro_protocol::shared::Flag,
    /// Accepted theme selection policy and renderer overrides.
    pub(crate) theme_runtime: ThemeRuntimeConfig,
    /// Active authored or sibling theme name, independent of palette rendering.
    pub(crate) theme_name: String,
    /// Last observed host appearance.
    pub(crate) host_terminal_appearance: Option<crate::terminal_theme::HostAppearance>,
    /// Whether a host report takes precedence over background inference.
    pub(crate) host_terminal_appearance_explicit: maestro_protocol::shared::Flag,
    /// Host default and palette colours propagated to live panes.
    pub(crate) host_terminal_theme: crate::terminal_theme::TerminalTheme,
    /// Whether interactive creation waits for a supplied workspace name.
    pub(crate) prompt_new_workspace_name: maestro_protocol::shared::Flag,
    /// Directory retained while the input owner collects a new workspace name.
    pub(crate) pending_workspace_create_cwd: Option<std::path::PathBuf>,
    /// Live ordering selection for the agent panel.
    pub(crate) agent_panel_sort: AgentPanelSort,
    /// Current sidebar collapse state, initialized only on construction.
    pub(crate) sidebar_collapsed: maestro_protocol::shared::Flag,
    /// Whether outer focus gain requests a full host redraw.
    pub(crate) redraw_on_focus_gained: maestro_protocol::shared::Flag,
    /// Ordered retained right-side status segments.
    pub(crate) tab_bar_right: Vec<TabBarStatusSegment>,
    /// Printable spacing between status segments.
    pub(crate) tab_bar_right_separator: String,
    /// Sole owner of workspace topology and public identity allocation.
    pub topology: WorkspaceTopology,
    /// Retained terminal metadata keyed by the attached terminal identity.
    pub(crate) terminals: HashMap<TerminalId, TerminalState>,
    /// Compiled detection snapshot shared with pane runtimes.
    pub manifests: Arc<RwLock<ManifestCache>>,
    /// Active detection sources and versions from the shared snapshot.
    pub(crate) agent_manifest_summaries: Vec<AgentManifestSummary>,
    /// Diagnostics from the latest detection catalogue update.
    pub(crate) agent_manifest_update_status: ManifestUpdateStatus,
    /// Registered pane provenance removed with its owning pane.
    pub(crate) action_panes: HashMap<PaneId, ActionPaneRecord>,
    /// Detached metadata identities awaiting runtime shutdown by the loop.
    pub(crate) terminal_runtime_shutdowns: Vec<TerminalId>,
    /// Active workspace display position, absent when no workspace is active.
    pub(crate) active: Option<usize>,
    /// Workspace selection position for navigation.
    pub(crate) selected: usize,
    /// Structural changes awaiting snapshot scheduling.
    pub(crate) session_dirty: bool,
    /// Background notification preferences applied independently of sound.
    pub(crate) toast_config: crate::settings::model::ToastConfig,
    /// Per-agent sound admission preferences.
    pub(crate) sound: crate::settings::audio::SoundConfig,
    /// Last known outer terminal focus, absent before the first report.
    pub(crate) outer_terminal_focus: Option<bool>,
    /// Clipboard acknowledgement kept separately from agent notification content.
    pub(crate) copy_feedback: Option<CopyFeedback>,
    /// Most recent workspace notification.
    pub(crate) toast: Option<ToastNotification>,
    /// Available application version reported by the update producer.
    pub(crate) update_available: Option<String>,
    /// Install command supplied by the update producer.
    pub(crate) update_install_command: String,
    /// Whether release notes are available for the reported version.
    pub(crate) latest_release_notes_available: bool,
    /// Whether automatic update prompting has been dismissed.
    pub(crate) update_dismissed: bool,
    /// Delayed transitions revalidated before delivery.
    pub(crate) pending_agent_notifications: HashMap<PaneId, PendingAgentNotification>,
    /// Sequence shared by effective agent state changes.
    pub(crate) next_agent_state_change_seq: u64,
    /// Whether implicit parent-group closes require confirmation.
    pub(crate) confirm_close: maestro_protocol::shared::Flag,
    /// Current interaction context.
    pub(crate) mode: Mode,
    /// Keyboard copy pane retained across focus navigation.
    pub(crate) copy_mode: Option<CopyModeState>,
    /// Current terminal text selection.
    pub(crate) selection: Option<crate::selection::Selection>,
    /// Pending drag geometry cleared with its selection.
    pub(crate) selection_autoscroll: Option<SelectionAutoscroll>,
    /// Previous logical focus for last-pane navigation.
    pub(crate) previous_pane_focus: Option<PaneFocusTarget>,
    /// Historical numeric spellings mapped to their retained pane identity.
    pub(crate) pane_id_aliases: HashMap<u32, PaneId>,
    /// Historical public spellings mapped to their retained pane identity.
    pub(crate) public_pane_id_aliases: HashMap<String, PaneId>,
    /// Virtual terminal dimensions in columns and rows before a view attaches.
    pub(crate) headless_size: (u16, u16),
    /// Most recently supplied pane geometry, in view order.
    pub(crate) pane_infos: Vec<PaneInfo>,
}

impl DaemonState {
    /// Creates empty daemon state using accepted settings and an identity clock.
    #[must_use]
    pub fn new(
        config: &Config,
        clock: impl FnMut() -> u128 + 'static,
        manifests: Arc<RwLock<ManifestCache>>,
    ) -> Self {
        let (sidebar_min, sidebar_max) = config.sidebar_bounds();
        let theme_runtime = ThemeRuntimeConfig::new(config, true);
        let agent_manifest_summaries = manifests
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .manifest_summaries();
        Self {
            should_quit: maestro_protocol::shared::Flag(false),
            sidebar_width: config.ui.sidebar_width.clamp(sidebar_min, sidebar_max),
            sidebar_width_source: SidebarWidthSource::ConfigDefault,
            sidebar_section_split: 0.5,
            collapsed_space_keys: std::collections::HashSet::new(),
            agent_panel_scroll: 0,
            config_diagnostic: None,
            request_client_config_reload: maestro_protocol::shared::Flag(false),
            theme_name: theme_runtime.effective_name(None).into(),
            theme_runtime,
            host_terminal_appearance: None,
            host_terminal_appearance_explicit: maestro_protocol::shared::Flag(false),
            host_terminal_theme: crate::terminal_theme::TerminalTheme::default(),
            prompt_new_workspace_name: config.ui.prompt_new_workspace_name,
            pending_workspace_create_cwd: None,
            agent_panel_sort: config.ui.agent_panel_sort,
            sidebar_collapsed: config.ui.sidebar_start_collapsed,
            redraw_on_focus_gained: config.ui.redraw_on_focus_gained,
            tab_bar_right: Vec::new(),
            tab_bar_right_separator: " ".into(),
            topology: WorkspaceTopology::new(clock),
            terminals: HashMap::new(),
            manifests,
            agent_manifest_summaries,
            agent_manifest_update_status: ManifestUpdateStatus::default(),
            action_panes: HashMap::new(),
            terminal_runtime_shutdowns: Vec::new(),
            active: None,
            selected: 0,
            session_dirty: false,
            previous_pane_focus: None,
            toast_config: config.ui.toast.clone(),
            sound: config.ui.sound.clone(),
            outer_terminal_focus: None,
            toast: None,
            copy_feedback: None,
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

    /// Runs a change to the saved workspace structure and marks the session for saving.
    ///
    /// The mark is set once `edit` returns, so every change that must outlive the daemon goes
    /// through here instead of setting the mark by hand.
    pub(crate) fn edit_topology<R>(&mut self, edit: impl FnOnce(&mut Self) -> R) -> R {
        let result = edit(self);
        self.session_dirty = true;
        result
    }

    /// Marks the session for saving when a change that may or may not have happened did.
    pub(crate) fn mark_session_dirty_if(&mut self, changed: bool) {
        self.session_dirty |= changed;
    }

    /// Reads and clears the save mark, reporting whether a change awaits a snapshot.
    pub(crate) fn take_session_dirty(&mut self) -> bool {
        std::mem::take(&mut self.session_dirty)
    }

    /// Whether the sidebar starts collapsed, as the configuration set it at construction.
    #[must_use]
    pub fn sidebar_collapsed(&self) -> bool {
        self.sidebar_collapsed.0
    }

    /// Region in which panes are laid out: the headless surface at its configured size.
    #[must_use]
    pub fn view_area(&self) -> Rect {
        Rect::new(0, 0, self.headless_size.0, self.headless_size.1)
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

#[cfg(test)]
mod tests;
