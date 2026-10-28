//! Caller-supplied launch inputs shared by topology creation and shell replacement.

use super::{App, state::DaemonState};
use crate::{
    api::event_hub::EventHub,
    events::AppEvent,
    pane::runtime::{AgentDetection, PaneRuntimeConfig},
    render_signal::RenderSignal,
    settings::{io::AcceptedAreas, model::Config},
    terminal::runtime_registry::TerminalRuntimeRegistry,
    terminal_theme::{HostAppearance, TerminalTheme},
};
/// The detection snapshot type of [`PaneLaunchInputs::manifests`].
pub use maestro_detection::manifest::ManifestCache;
use maestro_process::pty::PtySize;
/// The child-setup entry and process detection policy of [`PaneLaunchInputs`].
pub use maestro_process::pty::{PtySetupEntry, foreground::ProcessDetectionMode, setup_entry};
use maestro_workspace::layout::PaneId;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tokio::sync::{Notify, mpsc};

/// Shared runtime adapters retained without a placeholder pane identity or geometry.
pub struct PaneLaunchInputs {
    /// Disable saved sessions and automatic background checks for this daemon lifetime.
    pub no_session: bool,
    /// Caller executable implementing child-side terminal setup.
    pub setup_entry: PtySetupEntry,
    /// Daemon socket advertised to child commands.
    pub socket: PathBuf,
    /// Optional application executable advertised to child commands.
    pub executable: Option<PathBuf>,
    /// Detection snapshot shared with daemon metadata and every runtime.
    pub manifests: Arc<RwLock<ManifestCache>>,
    /// Single internal event destination for every pane.
    pub events: mpsc::Sender<AppEvent>,
    /// Wakeup shared with the daemon loop.
    pub render_notify: Arc<Notify>,
    /// Retained rendering origins shared with pane readers.
    pub render_dirty: Arc<RenderSignal>,
    /// Host terminal colours supplied independently of theme-name selection.
    pub theme: TerminalTheme,
    /// Explicit host appearance, when reported.
    pub appearance: Option<HostAppearance>,
    /// Native foreground process observation policy.
    pub process_detection_mode: ProcessDetectionMode,
}

impl PaneLaunchInputs {
    /// Binds one real pane and current geometry to the common launch adapters.
    #[must_use]
    pub fn runtime_config(
        &self,
        config: &Config,
        pane_id: PaneId,
        size: (u16, u16),
    ) -> PaneRuntimeConfig {
        PaneRuntimeConfig {
            pane_id,
            size: PtySize {
                rows: size.0,
                cols: size.1,
                cell_width_px: 0,
                cell_height_px: 0,
            },
            scrollback_limit_bytes: config.advanced.scrollback_limit_bytes,
            theme: self.theme,
            appearance: self.appearance,
            events: self.events.clone(),
            render_notify: Arc::clone(&self.render_notify),
            render_dirty: Arc::clone(&self.render_dirty),
            manifests: Arc::clone(&self.manifests),
            process_detection_mode: self.process_detection_mode,
            agent_detection: AgentDetection::Enabled,
            setup_entry: self.setup_entry.clone(),
            socket: self.socket.clone(),
            executable: self.executable.clone(),
        }
    }
}

impl<C: FnMut() -> u128> App<C> {
    /// Constructs from loaded preferences whenever a loaded result is available.
    /// Retains the loader's accepted documents for the first subsequent reload.
    pub fn from_loaded(
        loaded: &crate::settings::model::LoadedConfig,
        clock: C,
        launch: PaneLaunchInputs,
        event_rx: mpsc::Receiver<AppEvent>,
        event_hub: EventHub,
    ) -> Self {
        let mut app = Self::new(&loaded.config, clock, launch, event_rx, event_hub);
        app.accepted_config_areas.clone_from(&loaded.accepted_areas);
        app
    }

    /// Constructs one daemon owner from accepted preferences and supplied runtime adapters.
    pub fn new(
        config: &Config,
        clock: C,
        launch: PaneLaunchInputs,
        event_rx: mpsc::Receiver<AppEvent>,
        event_hub: EventHub,
    ) -> Self {
        let mut accepted = config.clone();
        (accepted.ui.sidebar_min_width, accepted.ui.sidebar_max_width) = config.sidebar_bounds();
        let config = &accepted;
        let mut app = Self {
            window_title_template: None,
            session_save_deadline: None,
            session_save_thread: None,
            next_resize_poll: std::time::Instant::now() + std::time::Duration::from_millis(100),
            last_render_at: None,
            last_presentation_at: None,
            toast_deadline: None,
            last_api_notification_at: None,
            event_rx,
            next_auto_update_check: None,
            next_agent_manifest_update_check: None,
            config_diagnostic_deadline: None,
            config_reloaded_from_disk: false,
            clear_history_after_reload: false,
            copy_feedback_deadline: None,
            tab_bar_status: super::tab_bar_status::TabBarStatus::default(),
            state: DaemonState::new(config, clock, Arc::clone(&launch.manifests)),
            config: accepted.clone(),
            accepted_config_areas: AcceptedAreas::default(),
            launch,
            terminal_runtimes: TerminalRuntimeRegistry::new(),
            worktree_operations: super::api::WorktreeOperations::default(),
            event_hub,
            overlay_panes: HashMap::new(),
        };
        app.state.host_terminal_theme = app.launch.theme;
        if let Some(appearance) = app.launch.appearance {
            app.set_host_terminal_appearance(appearance, true);
        }
        app.configure_window_title(&config.ui.window_title);
        app.configure_tab_bar_status(&config.ui.tab_bar_right, &config.ui.tab_bar_right_separator);
        app
    }
}
