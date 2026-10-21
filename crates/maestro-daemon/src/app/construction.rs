//! Caller-supplied launch inputs shared by topology creation and shell replacement.

use super::{App, state::DaemonState};
use crate::{
    api::event_hub::EventHub,
    events::AppEvent,
    pane::runtime::{AgentDetection, PaneRuntimeConfig},
    render_signal::RenderSignal,
    settings::model::Config,
    terminal::runtime_registry::TerminalRuntimeRegistry,
    terminal_theme::{HostAppearance, TerminalTheme},
};
use maestro_detection::manifest::ManifestCache;
use maestro_process::pty::{PtySetupEntry, PtySize, foreground::ProcessDetectionMode};
use maestro_workspace::layout::PaneId;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};
use tokio::sync::{Notify, mpsc};

/// Shared runtime adapters retained without a placeholder pane identity or geometry.
pub struct PaneLaunchInputs {
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
    /// Constructs one daemon owner from accepted preferences and supplied runtime adapters.
    pub fn new(config: &Config, clock: C, launch: PaneLaunchInputs, event_hub: EventHub) -> Self {
        let mut app = Self {
            tab_bar_status: super::tab_bar_status::TabBarStatus::default(),
            state: DaemonState::new(config, clock, Arc::clone(&launch.manifests)),
            config: config.clone(),
            launch,
            terminal_runtimes: TerminalRuntimeRegistry::new(),
            event_hub,
            overlay_panes: HashMap::new(),
        };
        app.configure_tab_bar_status(&config.ui.tab_bar_right, &config.ui.tab_bar_right_separator);
        app
    }
}
