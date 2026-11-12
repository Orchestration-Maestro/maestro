//! Retained theme-name policy without renderer-owned palette construction.

use crate::{
    settings::{appearance::CustomThemeColors, model::Config},
    terminal_theme::HostAppearance,
};

/// Accepted theme selections and overrides applied after the active base theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRuntimeConfig {
    /// Authored manual selection, including its original spelling.
    pub manual_name: String,
    /// Selection used for a dark host appearance.
    pub dark_name: String,
    /// Selection used for a light host appearance.
    pub light_name: String,
    /// Whether host appearance selects a sibling theme.
    pub auto_switch: bool,
    /// Authored colours retained for the renderer.
    pub custom: Option<CustomThemeColors>,
    /// Legacy interface accent when no custom accent supersedes it.
    pub legacy_accent: Option<String>,
}

impl ThemeRuntimeConfig {
    /// Resolves sibling defaults while preserving authored selections and overrides.
    #[must_use]
    pub fn new(config: &Config, use_legacy_ui_accent: bool) -> Self {
        let manual_name = config
            .theme
            .name
            .clone()
            .unwrap_or_else(|| "catppuccin".into());
        let (dark, light) = sibling_theme_names(&manual_name);
        Self {
            manual_name: manual_name.clone(),
            dark_name: config
                .theme
                .dark_name
                .clone()
                .unwrap_or_else(|| dark.into()),
            light_name: config
                .theme
                .light_name
                .clone()
                .unwrap_or_else(|| light.into()),
            auto_switch: config.theme.auto_switch,
            custom: config.theme.custom.clone(),
            legacy_accent: (use_legacy_ui_accent
                && config.ui.accent != "cyan"
                && config
                    .theme
                    .custom
                    .as_ref()
                    .and_then(|c| c.accent.as_ref())
                    .is_none())
            .then(|| config.ui.accent.clone()),
        }
    }

    /// Selects the active name without interpreting palette colours.
    #[must_use]
    pub fn effective_name(&self, appearance: Option<HostAppearance>) -> &str {
        self.effective_selection(appearance).0
    }

    /// Selects the active name and the built-in theme used when it is unknown.
    #[must_use]
    pub fn effective_selection(&self, appearance: Option<HostAppearance>) -> (&str, &'static str) {
        if !self.auto_switch {
            (&self.manual_name, "catppuccin")
        } else if appearance == Some(HostAppearance::Light) {
            (&self.light_name, "catppuccin-latte")
        } else {
            (&self.dark_name, "catppuccin")
        }
    }
}

/// Resolves supported aliases to the corresponding dark and light pair.
fn sibling_theme_names(name: &str) -> (&str, &str) {
    use crate::settings::appearance::canonical_theme_name;
    match canonical_theme_name(name) {
        Some("catppuccin" | "catppuccin-latte") => ("catppuccin", "catppuccin-latte"),
        Some("tokyo-night" | "tokyo-night-day") => ("tokyo-night", "tokyo-night-day"),
        Some("gruvbox" | "gruvbox-light") => ("gruvbox", "gruvbox-light"),
        Some("one-dark" | "one-light") => ("one-dark", "one-light"),
        Some("solarized" | "solarized-light") => ("solarized", "solarized-light"),
        Some("kanagawa" | "kanagawa-lotus") => ("kanagawa", "kanagawa-lotus"),
        Some("rose-pine" | "rose-pine-dawn") => ("rose-pine", "rose-pine-dawn"),
        _ => (name, name),
    }
}
