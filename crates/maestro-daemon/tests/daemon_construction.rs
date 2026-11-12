//! Initial daemon presentation preferences come from the accepted configuration.
#![cfg(test)]
#![cfg(target_os = "linux")]

use maestro_daemon::settings::model::Config;
use maestro_daemon::state::test_support::api::daemon_with;

#[test]
fn theme_siblings_preserve_aliases_custom_names_and_authored_pair_overrides() {
    use maestro_daemon::{state::model::ThemeRuntimeConfig, terminal_theme::HostAppearance};
    let mut config = Config::default();
    config.theme.auto_switch = true;
    for (name, dark, light) in [
        ("Catppuccin Mocha", "catppuccin", "catppuccin-latte"),
        ("tokyonight-day", "tokyo-night", "tokyo-night-day"),
        ("gruvbox-dark", "gruvbox", "gruvbox-light"),
        ("onelight", "one-dark", "one-light"),
        ("solarized_dark", "solarized", "solarized-light"),
        ("lotus", "kanagawa", "kanagawa-lotus"),
        ("dawn", "rose-pine", "rose-pine-dawn"),
        ("NORD", "NORD", "NORD"),
        ("authored", "authored", "authored"),
    ] {
        config.theme.name = Some(name.into());
        let runtime = ThemeRuntimeConfig::new(&config, true);
        assert_eq!(runtime.effective_name(None), dark);
        assert_eq!(runtime.effective_name(Some(HostAppearance::Light)), light);
        assert_eq!(runtime.manual_name, name);
    }
    config.theme.dark_name = Some("authored-dark".into());
    config.theme.light_name = Some("authored-light".into());
    config.ui.accent = "red".into();
    let runtime = ThemeRuntimeConfig::new(&config, true);
    assert_eq!(runtime.effective_name(None), "authored-dark");
    assert_eq!(
        runtime.effective_name(Some(HostAppearance::Light)),
        "authored-light"
    );
    assert_eq!(runtime.legacy_accent.as_deref(), Some("red"));
    assert!(
        ThemeRuntimeConfig::new(&config, false)
            .legacy_accent
            .is_none()
    );
}
#[test]
fn daemon_app_new_falls_back_to_default_bounds_on_inverted_config() {
    let mut config = Config::default();
    config.ui.sidebar_min_width = 50;
    config.ui.sidebar_max_width = 30;
    let app = daemon_with(&config).unwrap();
    assert_eq!(app.config.ui.sidebar_min_width, 18);
    assert_eq!(app.config.ui.sidebar_max_width, 36);
}
