//! The chrome palette follows the daemon's theme selection and host appearance.
#![cfg(target_os = "linux")]

use maestro_app::daemon::{Config, CustomThemeColors, HostAppearance, ThemeRuntimeConfig};
use maestro_multiplexer::palette::{Palette, resolve_effective_theme};
use ratatui::style::Color;

#[test]
fn daemon_theme_auto_switch_is_opt_in_and_preserves_manual_default() {
    let mut config = Config::default();
    config.theme.name = Some("tokyo-night".to_owned());
    let runtime = ThemeRuntimeConfig::new(&config, true);

    assert!(!runtime.auto_switch);
    for appearance in [None, Some(HostAppearance::Light)] {
        let (palette, name) = resolve_effective_theme(&runtime, appearance);
        assert_eq!(name, "tokyo-night");
        assert_eq!(palette, Palette::tokyo_night());
    }
}

#[test]
fn daemon_theme_auto_switch_uses_sibling_map_and_explicit_appearance() {
    let mut config = Config::default();
    config.theme.name = Some("tokyo-night".to_owned());
    config.theme.auto_switch = true;
    let runtime = ThemeRuntimeConfig::new(&config, true);

    let (palette, name) = resolve_effective_theme(&runtime, None);
    assert_eq!(name, "tokyo-night");
    assert_eq!(palette, Palette::tokyo_night());
    let (palette, name) = resolve_effective_theme(&runtime, Some(HostAppearance::Light));
    assert_eq!(name, "tokyo-night-day");
    assert_eq!(palette, Palette::tokyo_night_day());
}

#[test]
fn daemon_theme_auto_switch_applies_custom_overrides_after_active_base() {
    let mut config = Config::default();
    config.theme.name = Some("gruvbox".to_owned());
    config.theme.auto_switch = true;
    config.theme.custom = Some(CustomThemeColors {
        accent: Some("#010203".to_owned()),
        ..CustomThemeColors::default()
    });
    let runtime = ThemeRuntimeConfig::new(&config, true);

    let (palette, name) = resolve_effective_theme(&runtime, Some(HostAppearance::Light));
    assert_eq!(name, "gruvbox-light");
    assert_eq!(palette.accent, Color::Rgb(1, 2, 3));
    assert_eq!(palette.panel_bg, Palette::gruvbox_light().panel_bg);
}

#[test]
fn daemon_theme_unknown_light_selection_falls_back_to_the_light_default() {
    let mut config = Config::default();
    config.theme.auto_switch = true;
    config.theme.light_name = Some("lattee".to_owned());
    config.theme.dark_name = Some("tokio-night".to_owned());
    let runtime = ThemeRuntimeConfig::new(&config, true);

    let (palette, name) = resolve_effective_theme(&runtime, Some(HostAppearance::Light));
    assert_eq!(name, "lattee");
    assert_eq!(palette, Palette::catppuccin_latte());
    let (palette, name) = resolve_effective_theme(&runtime, Some(HostAppearance::Dark));
    assert_eq!(name, "tokio-night");
    assert_eq!(palette, Palette::catppuccin());
}
