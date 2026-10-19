//! Workspace theme preference behavior.

use super::super::theme::{ThemeConfig, canonical_theme_name};
use serde_json::json;

#[test]
fn theme_selection_keeps_supplied_name() {
    let config: ThemeConfig = serde_json::from_value(json!({"name":"tokyo-night"})).unwrap();
    assert_eq!(config.name.as_deref(), Some("tokyo-night"));
}

#[test]
fn theme_absence_keeps_default_selection() {
    let config: ThemeConfig = serde_json::from_value(json!({})).unwrap();
    assert_eq!(config.name, None);
    assert!(!config.auto_switch);
    assert_eq!(config.dark_name, None);
    assert_eq!(config.light_name, None);
    assert_eq!(config.custom, None);
}

#[test]
fn theme_switch_options_remain_independent() {
    let config: ThemeConfig =
        serde_json::from_value(json!({"autoSwitch":true,"darkName":"nord","lightName":"latte"}))
            .unwrap();
    assert!(config.auto_switch);
    assert_eq!(config.dark_name.as_deref(), Some("nord"));
    assert_eq!(config.light_name.as_deref(), Some("latte"));
    let config: ThemeConfig = serde_json::from_value(json!({"darkName":"dracula"})).unwrap();
    assert!(!config.auto_switch);
    assert_eq!(config.dark_name.as_deref(), Some("dracula"));
    assert_eq!(config.light_name, None);
    let config: ThemeConfig = serde_json::from_value(json!({"lightName":"one-light"})).unwrap();
    assert_eq!(config.dark_name, None);
    assert_eq!(config.light_name.as_deref(), Some("one-light"));
}

#[test]
fn theme_aliases_resolve_to_canonical_names() {
    use super::super::theme::THEME_NAMES;
    assert_eq!(THEME_NAMES.len(), 18);
    for name in THEME_NAMES {
        assert_eq!(canonical_theme_name(name), Some(*name));
    }
    for (alias, expected) in [
        ("catppuccin-mocha", "catppuccin"),
        ("latte", "catppuccin-latte"),
        ("light", "catppuccin-latte"),
        ("tokyonight", "tokyo-night"),
        ("tokyo-day", "tokyo-night-day"),
        ("tokyonight-day", "tokyo-night-day"),
        ("gruvbox-dark", "gruvbox"),
        ("onedark", "one-dark"),
        ("onelight", "one-light"),
        ("solarized-dark", "solarized"),
        ("lotus", "kanagawa-lotus"),
        ("rosepine", "rose-pine"),
        ("rosepine-dawn", "rose-pine-dawn"),
        ("dawn", "rose-pine-dawn"),
        ("TOKYO_NIGHT", "tokyo-night"),
        ("One Light", "one-light"),
    ] {
        assert_eq!(canonical_theme_name(alias), Some(expected), "{alias}");
    }
    assert_eq!(canonical_theme_name("tokyo-night-neighbour"), None);
    assert_eq!(canonical_theme_name(" nord "), None);
}

#[test]
fn theme_unknown_names_name_each_fallback() {
    let config: ThemeConfig = serde_json::from_value(
        json!({"name":"no-main","darkName":"no-dark","lightName":"no-light"}),
    )
    .unwrap();
    let valid = "catppuccin, catppuccin-latte, terminal, tokyo-night, tokyo-night-day, dracula, nord, gruvbox, gruvbox-light, one-dark, one-light, solarized, solarized-light, kanagawa, kanagawa-lotus, rose-pine, rose-pine-dawn, vesper";
    assert_eq!(
        config.diagnostics(),
        [
            format!(
                "unknown theme name multiplexer.theme.name = \"no-main\"; using \"catppuccin\"; valid themes: {valid}"
            ),
            format!(
                "unknown theme name multiplexer.theme.darkName = \"no-dark\"; using \"catppuccin\"; valid themes: {valid}"
            ),
            format!(
                "unknown theme name multiplexer.theme.lightName = \"no-light\"; using \"catppuccin-latte\"; valid themes: {valid}"
            ),
        ]
    );
    let valid: ThemeConfig = serde_json::from_value(json!({"name":"latte"})).unwrap();
    assert!(valid.diagnostics().is_empty());
}

#[test]
fn theme_custom_tokens_keep_all_overrides() {
    let config: ThemeConfig = serde_json::from_value(json!({"custom":{
        "accent":"a", "panelBg":"b", "sidebarBg":"c", "activeRowBg":"d", "selectionBg":"e",
        "surface0":"f", "surface1":"g", "surfaceDim":"h", "overlay0":"i", "overlay1":"j",
        "text":"k", "subtext0":"l", "mauve":"m", "green":"n", "yellow":"o", "red":"p",
        "blue":"q", "teal":"r", "peach":"s"
    }}))
    .unwrap();
    let colors = config.custom.unwrap();
    assert_eq!(
        [
            colors.accent.as_deref(),
            colors.panel_bg.as_deref(),
            colors.sidebar_bg.as_deref(),
            colors.active_row_bg.as_deref(),
            colors.selection_bg.as_deref(),
            colors.surface0.as_deref(),
            colors.surface1.as_deref(),
            colors.surface_dim.as_deref(),
            colors.overlay0.as_deref(),
            colors.overlay1.as_deref(),
            colors.text.as_deref(),
            colors.subtext0.as_deref(),
            colors.mauve.as_deref(),
            colors.green.as_deref(),
            colors.yellow.as_deref(),
            colors.red.as_deref(),
            colors.blue.as_deref(),
            colors.teal.as_deref(),
            colors.peach.as_deref(),
        ],
        [
            Some("a"),
            Some("b"),
            Some("c"),
            Some("d"),
            Some("e"),
            Some("f"),
            Some("g"),
            Some("h"),
            Some("i"),
            Some("j"),
            Some("k"),
            Some("l"),
            Some("m"),
            Some("n"),
            Some("o"),
            Some("p"),
            Some("q"),
            Some("r"),
            Some("s"),
        ]
    );
    let empty: ThemeConfig = serde_json::from_value(json!({"custom":{}})).unwrap();
    assert_eq!(
        empty.custom.unwrap(),
        super::super::theme::CustomThemeColors::default()
    );
}
