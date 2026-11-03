//! Mutable daemon data with one topology allocator.

use crate::{settings::model::Config, terminal::state::TerminalState};
use maestro_detection::{
    manifest::{AgentManifestSummary, ManifestCache},
    manifest_update::ManifestUpdateStatus,
};
use maestro_workspace::terminal_id::TerminalId;
use maestro_workspace::{
    layout::{PaneId, PaneInfo, Rect},
    workspace::WorkspaceTopology,
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

/// Agent panel ordering uses the same vocabulary as the accepted preferences.
pub use crate::settings::model::AgentPanelSortConfig as AgentPanelSort;

mod notification;
pub use notification::{
    AgentNotificationDelivery, CopyFeedback, PendingAgentNotification, ToastKind,
    ToastNotification, ToastTarget,
};
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
pub struct DaemonState<C> {
    /// Whether the shared daemon loop should finish its owned shutdown.
    pub should_quit: maestro_protocol::shared::Flag,
    /// Live expanded sidebar width, independently resizable from its configured default.
    pub sidebar_width: u16,
    /// Persisted fraction separating sidebar sections.
    pub sidebar_section_split: f32,
    /// Persisted collapsed repository group identities.
    pub collapsed_space_keys: std::collections::HashSet<String>,
    /// Owner of the live sidebar width.
    pub sidebar_width_source: SidebarWidthSource,
    /// Current agent-panel scroll position, reset when row composition reloads.
    pub agent_panel_scroll: usize,
    /// Compact diagnostic for the most recently loaded settings document.
    pub config_diagnostic: Option<String>,
    /// Whether connected clients must reacquire accepted interface preferences.
    pub request_client_config_reload: maestro_protocol::shared::Flag,
    /// Accepted theme selection policy and renderer overrides.
    pub theme_runtime: ThemeRuntimeConfig,
    /// Active authored or sibling theme name, independent of palette rendering.
    pub theme_name: String,
    /// Last observed host appearance.
    pub host_terminal_appearance: Option<crate::terminal_theme::HostAppearance>,
    /// Whether a host report takes precedence over background inference.
    pub host_terminal_appearance_explicit: maestro_protocol::shared::Flag,
    /// Host default and palette colours propagated to live panes.
    pub host_terminal_theme: crate::terminal_theme::TerminalTheme,
    /// Whether interactive creation waits for a supplied workspace name.
    pub prompt_new_workspace_name: maestro_protocol::shared::Flag,
    /// Directory retained while the input owner collects a new workspace name.
    pub pending_workspace_create_cwd: Option<std::path::PathBuf>,
    /// Live ordering selection for the agent panel.
    pub agent_panel_sort: AgentPanelSort,
    /// Current sidebar collapse state, initialized only on construction.
    pub sidebar_collapsed: maestro_protocol::shared::Flag,
    /// Whether outer focus gain requests a full host redraw.
    pub redraw_on_focus_gained: maestro_protocol::shared::Flag,
    /// Ordered retained right-side status segments.
    pub tab_bar_right: Vec<TabBarStatusSegment>,
    /// Printable spacing between status segments.
    pub tab_bar_right_separator: String,
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
    /// Clipboard acknowledgement kept separately from agent notification content.
    pub copy_feedback: Option<CopyFeedback>,
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
