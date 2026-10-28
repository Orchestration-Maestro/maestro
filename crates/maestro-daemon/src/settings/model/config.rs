//! Composed workspace preferences and load results.

use std::path::Path;

use super::{
    AdvancedConfig, ExperimentalConfig, RemoteConfig, ServerConfig, SessionConfig, TerminalConfig,
    UiConfig, UpdateConfig, WorktreesConfig, validated_sidebar_bounds,
};
use crate::settings::{
    tab_bar::tab_bar_right_diagnostics, theme::ThemeConfig, window_title::window_title_diagnostics,
};
pub use maestro_protocol::shared::ConfigReloadStatus;

/// Owned workspace preferences, independent of their stored namespaces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config {
    /// Whether to show initial setup; absent means visible.
    pub onboarding: Option<bool>,
    /// Workspace color preferences.
    pub theme: ThemeConfig,
    /// New-pane shell and directory preferences.
    pub terminal: TerminalConfig,
    /// Saved-session restore preferences.
    pub session: SessionConfig,
    /// Unattached terminal dimensions.
    pub server: ServerConfig,
    /// Product and detection update preferences.
    pub update: UpdateConfig,
    /// Client interface preferences.
    pub ui: UiConfig,
    /// Worktree location preferences.
    pub worktrees: WorktreesConfig,
    /// Terminal memory preferences.
    pub advanced: AdvancedConfig,
    /// Optional client and daemon capabilities.
    pub experimental: ExperimentalConfig,
    /// Remote connection preferences.
    pub remote: RemoteConfig,
}

impl Config {
    /// Whether initial setup should be displayed.
    #[must_use]
    pub fn should_show_onboarding(&self) -> bool {
        self.onboarding.unwrap_or(true)
    }

    /// Collect actionable warnings using the supplied sound-file root.
    #[must_use]
    pub fn collect_diagnostics(&self, agent_dir: &Path) -> Vec<String> {
        self.theme
            .diagnostics()
            .into_iter()
            .chain(self.ui.sound.diagnostics(agent_dir))
            .chain(tab_bar_right_diagnostics(&self.ui.tab_bar_right))
            .chain(window_title_diagnostics(&self.ui.window_title))
            .chain(self.invalid_sidebar_bounds_diagnostic())
            .chain(self.server.invalid_headless_size_diagnostic())
            .collect()
    }

    /// Validated unattached terminal dimensions.
    #[must_use]
    pub fn headless_size(&self) -> (u16, u16) {
        self.server.headless_size()
    }

    /// Accepts startup sidebar bounds, falling back to interface defaults when inverted.
    #[must_use]
    pub fn sidebar_bounds(&self) -> (u16, u16) {
        validated_sidebar_bounds(self.ui.sidebar_min_width, self.ui.sidebar_max_width)
            .unwrap_or_else(|| {
                let defaults = super::UiConfig::default();
                (defaults.sidebar_min_width, defaults.sidebar_max_width)
            })
    }

    /// Explain inverted sidebar width bounds before clamping.
    #[must_use]
    pub fn invalid_sidebar_bounds_diagnostic(&self) -> Option<String> {
        validated_sidebar_bounds(self.ui.sidebar_min_width, self.ui.sidebar_max_width)
            .is_none()
            .then(|| {
                format!(
                    "multiplexer.sidebarMinWidth ({}) is greater than sidebarMaxWidth ({})",
                    self.ui.sidebar_min_width, self.ui.sidebar_max_width
                )
            })
    }
}

/// Decoded preferences with warnings and rejected semantic areas.
#[derive(Debug, Clone)]
pub struct LoadedConfig {
    /// Accepted values, with defaults for invalid areas.
    pub config: Config,
    /// Actionable warnings.
    pub diagnostics: Vec<String>,
    /// Semantic areas whose typed decoding failed.
    pub invalid_sections: Vec<String>,
    /// Last successfully decoded plain document for each independent area.
    pub accepted_areas: crate::settings::io::AcceptedAreas,
}

/// Result of applying a live preference reload.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct ConfigReloadReport {
    /// Whether all, some or none of the requested areas applied.
    pub status: ConfigReloadStatus,
    /// Warnings and retained-value explanations.
    pub diagnostics: Vec<String>,
}
