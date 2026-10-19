//! Built-in workspace theme selection and authored color overrides.

use serde::Deserialize;

/// Canonical workspace theme names.
pub const THEME_NAMES: &[&str] = &[
    "catppuccin",
    "catppuccin-latte",
    "terminal",
    "tokyo-night",
    "tokyo-night-day",
    "dracula",
    "nord",
    "gruvbox",
    "gruvbox-light",
    "one-dark",
    "one-light",
    "solarized",
    "solarized-light",
    "kanagawa",
    "kanagawa-lotus",
    "rose-pine",
    "rose-pine-dawn",
    "vesper",
];

/// Resolve a supported alias without changing the authored setting.
#[must_use]
pub fn canonical_theme_name(name: &str) -> Option<&'static str> {
    match name.to_lowercase().replace([' ', '_'], "-").as_str() {
        "catppuccin" | "catppuccin-mocha" => Some("catppuccin"),
        "catppuccin-latte" | "latte" | "light" => Some("catppuccin-latte"),
        "terminal" => Some("terminal"),
        "tokyo-night" | "tokyonight" => Some("tokyo-night"),
        "tokyo-night-day" | "tokyo-day" | "tokyonight-day" => Some("tokyo-night-day"),
        "dracula" => Some("dracula"),
        "nord" => Some("nord"),
        "gruvbox" | "gruvbox-dark" => Some("gruvbox"),
        "gruvbox-light" => Some("gruvbox-light"),
        "one-dark" | "onedark" => Some("one-dark"),
        "one-light" | "onelight" => Some("one-light"),
        "solarized" | "solarized-dark" => Some("solarized"),
        "solarized-light" => Some("solarized-light"),
        "kanagawa" => Some("kanagawa"),
        "kanagawa-lotus" | "lotus" => Some("kanagawa-lotus"),
        "rose-pine" | "rosepine" => Some("rose-pine"),
        "rose-pine-dawn" | "rosepine-dawn" | "dawn" => Some("rose-pine-dawn"),
        "vesper" => Some("vesper"),
        _ => None,
    }
}

/// Built-in selection and independent host-appearance preferences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThemeConfig {
    /// Selected name; absence uses catppuccin.
    pub name: Option<String>,
    /// Follow the host's light or dark appearance.
    pub auto_switch: bool,
    /// Dark-appearance selection; absence uses catppuccin.
    pub dark_name: Option<String>,
    /// Light-appearance selection; absence uses catppuccin-latte.
    pub light_name: Option<String>,
    /// Authored token overrides layered on the selected theme.
    pub custom: Option<CustomThemeColors>,
}

impl ThemeConfig {
    /// Explain unknown names with each field's fallback and the valid choices.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<String> {
        let valid = THEME_NAMES.join(", ");
        [
            ("multiplexer.theme.name", self.name.as_deref(), "catppuccin"),
            (
                "multiplexer.theme.darkName",
                self.dark_name.as_deref(),
                "catppuccin",
            ),
            (
                "multiplexer.theme.lightName",
                self.light_name.as_deref(),
                "catppuccin-latte",
            ),
        ]
        .into_iter()
        .filter_map(|(field, value, fallback)| {
            let value = value?;
            canonical_theme_name(value).is_none().then(|| format!(
                "unknown theme name {field} = {value:?}; using {fallback:?}; valid themes: {valid}"
            ))
        })
        .collect()
    }
}

/// Optional authored color strings; rendering interprets them separately.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomThemeColors {
    /// Accent color.
    pub accent: Option<String>,
    /// Panel background.
    pub panel_bg: Option<String>,
    /// Sidebar background.
    pub sidebar_bg: Option<String>,
    /// Active row background.
    pub active_row_bg: Option<String>,
    /// Selection background.
    pub selection_bg: Option<String>,
    /// First surface color.
    pub surface0: Option<String>,
    /// Second surface color.
    pub surface1: Option<String>,
    /// Dimmed surface color.
    pub surface_dim: Option<String>,
    /// First overlay color.
    pub overlay0: Option<String>,
    /// Second overlay color.
    pub overlay1: Option<String>,
    /// Main text color.
    pub text: Option<String>,
    /// Secondary text color.
    pub subtext0: Option<String>,
    /// Mauve palette color.
    pub mauve: Option<String>,
    /// Green palette color.
    pub green: Option<String>,
    /// Yellow palette color.
    pub yellow: Option<String>,
    /// Red palette color.
    pub red: Option<String>,
    /// Blue palette color.
    pub blue: Option<String>,
    /// Teal palette color.
    pub teal: Option<String>,
    /// Peach palette color.
    pub peach: Option<String>,
}
