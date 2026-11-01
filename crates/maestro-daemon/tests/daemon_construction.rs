//! Initial daemon presentation preferences come from the accepted configuration.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support::launch;

use maestro_daemon::{
    api::event_hub::EventHub,
    app::{App, state::AgentPanelSort},
    settings::model::{AgentPanelSortConfig, Config},
};
use std::sync::{Arc, RwLock};

/// Constructs the real daemon owner without creating a workspace or native child.
fn app(config: &Config) -> App<fn() -> u128> {
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(&[], manifests);
    App::new(
        config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    )
}

#[test]
fn construction_uses_configured_agent_panel_sort() {
    let mut config = Config::default();
    config.ui.agent_panel_sort = AgentPanelSortConfig::Priority;
    assert_eq!(
        app(&config).state.agent_panel_sort,
        AgentPanelSort::Priority
    );
}

#[test]
fn construction_uses_configured_sidebar_state() {
    let mut config = Config::default();
    config.ui.sidebar_start_collapsed.0 = true;
    assert!(app(&config).state.sidebar_collapsed.0);
}

#[test]
fn construction_uses_redraw_on_focus_gained_config() {
    let mut config = Config::default();
    config.ui.redraw_on_focus_gained.0 = false;
    assert!(!app(&config).state.redraw_on_focus_gained.0);
}

#[test]
fn construction_uses_workspace_name_prompt_config() {
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    assert!(app(&config).state.prompt_new_workspace_name.0);
}

#[test]
fn pending_workspace_creation_blocks_default_until_cancelled() {
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    let mut app = app(&config);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            app.begin_workspace_create().await.unwrap();
            assert!(app.state.pending_workspace_create_cwd.is_some());
            assert!(!app.ensure_default_workspace());
            assert!(app.state.topology.workspaces.is_empty());
            assert!(app.terminal_runtimes.is_empty());
            app.cancel_workspace_create();
            assert!(app.state.topology.workspaces.is_empty());
            assert!(app.state.pending_workspace_create_cwd.is_none());
        });
}

#[test]
fn pending_creation_follows_navigation_selection_before_active_terminal() {
    use maestro_daemon::{app::state::Mode, terminal::state::TerminalState};
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    let mut app = app(&config);
    for cwd in ["/selected-first", "/selected-second"] {
        let (pane, ()) = app
            .state
            .topology
            .new_with_tab(cwd.into(), None, |_, _| Ok(()))
            .unwrap();
        let terminal = app
            .state
            .topology
            .workspaces
            .last()
            .unwrap()
            .terminal_id(pane)
            .unwrap()
            .clone();
        app.state
            .terminals
            .insert(terminal.clone(), TerminalState::new(terminal, cwd.into()));
    }
    app.state.selected = 1;
    app.state.active = Some(0);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            app.begin_workspace_create().await.unwrap();
            assert_eq!(
                app.state.pending_workspace_create_cwd.as_deref(),
                Some(std::path::Path::new("/selected-second"))
            );
            app.cancel_workspace_create();
            app.state.mode = Mode::Terminal;
            app.begin_workspace_create().await.unwrap();
            assert_eq!(
                app.state.pending_workspace_create_cwd.as_deref(),
                Some(std::path::Path::new("/selected-first"))
            );
        });
}

#[test]
fn pending_creation_retains_current_and_authored_directory_policies() {
    use maestro_daemon::settings::model::NewTerminalCwdConfig;
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    let mut app = app(&config);
    let current = std::env::current_dir().unwrap();
    let home = std::env::var_os("HOME");
    let tilde = home.as_ref().map_or_else(
        || std::path::PathBuf::from("~/directory"),
        |home| std::path::PathBuf::from(home).join("directory"),
    );
    let home = home.map_or_else(|| current.clone(), std::path::PathBuf::from);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for (policy, expected) in [
                (NewTerminalCwdConfig::Home, home.clone()),
                (NewTerminalCwdConfig::Current, current),
                (
                    NewTerminalCwdConfig::Path("/authored/../directory".into()),
                    "/authored/../directory".into(),
                ),
                (NewTerminalCwdConfig::Path("~/directory".into()), tilde),
            ] {
                app.config.terminal.new_cwd = policy;
                app.begin_workspace_create().await.unwrap();
                assert_eq!(app.state.pending_workspace_create_cwd, Some(expected));
                app.cancel_workspace_create();
            }
        });
}

#[test]
fn construction_keeps_manual_theme_policy() {
    let mut config = Config::default();
    config.theme.name = Some("tokyo-night".into());
    let mut app = app(&config);
    assert!(!app.state.theme_runtime.auto_switch);
    assert_eq!(app.state.theme_name, "tokyo-night");
    assert!(
        !app.set_host_terminal_appearance(
            maestro_daemon::terminal_theme::HostAppearance::Light,
            true,
        )
    );
    assert_eq!(app.state.theme_name, "tokyo-night");
}

#[test]
fn construction_selects_sibling_theme_for_explicit_host_appearance() {
    use maestro_daemon::terminal_theme::HostAppearance;
    let mut config = Config::default();
    config.theme.name = Some("tokyo-night".into());
    config.theme.auto_switch = true;
    let mut app = app(&config);
    assert_eq!(app.state.theme_name, "tokyo-night");
    assert!(app.set_host_terminal_appearance(HostAppearance::Light, true));
    assert_eq!(app.state.theme_name, "tokyo-night-day");
    assert!(!app.set_host_terminal_appearance(HostAppearance::Light, true));
    assert!(app.set_host_terminal_appearance(HostAppearance::Dark, true));
    assert_eq!(app.state.theme_name, "tokyo-night");
}

#[test]
fn construction_retains_custom_overrides_when_selecting_active_theme() {
    use maestro_daemon::{settings::theme::CustomThemeColors, terminal_theme::HostAppearance};
    let mut config = Config::default();
    config.theme.name = Some("gruvbox".into());
    config.theme.auto_switch = true;
    config.theme.custom = Some(CustomThemeColors {
        accent: Some("#010203".into()),
        ..CustomThemeColors::default()
    });
    config.ui.accent = "red".into();
    let mut app = app(&config);
    assert!(app.set_host_terminal_appearance(HostAppearance::Light, true));
    assert_eq!(app.state.theme_name, "gruvbox-light");
    assert_eq!(app.state.theme_runtime.custom, config.theme.custom);
    assert!(app.state.theme_runtime.legacy_accent.is_none());
}

#[test]
fn daemon_inferred_background_appearance_does_not_override_explicit_report() {
    use maestro_daemon::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
    let mut config = Config::default();
    config.theme.name = Some("catppuccin".into());
    config.theme.auto_switch = true;
    let mut app = app(&config);
    app.set_host_terminal_appearance(HostAppearance::Dark, true);
    let background = RgbColor {
        r: 255,
        g: 255,
        b: 255,
    };
    assert!(app.update_host_terminal_theme(DefaultColorKind::Background, background));
    assert_eq!(
        app.state.host_terminal_appearance,
        Some(HostAppearance::Dark)
    );
    assert_eq!(app.state.theme_name, "catppuccin");
    assert_eq!(app.state.host_terminal_theme.background, Some(background));
    assert!(!app.update_host_terminal_theme(DefaultColorKind::Background, background));
}

#[test]
fn construction_shares_supplied_host_observations_with_later_launches() {
    use maestro_daemon::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
    let mut config = Config::default();
    config.theme.auto_switch = true;
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (mut launch, receiver) = launch::launch_inputs(&[], manifests);
    launch.appearance = Some(HostAppearance::Light);
    launch.theme = launch.theme.with_color(
        DefaultColorKind::Background,
        RgbColor {
            r: 200,
            g: 201,
            b: 202,
        },
    );
    let mut app = App::new(&config, || 0, launch, receiver, EventHub::default());
    assert_eq!(app.state.theme_name, "catppuccin-latte");
    assert_eq!(app.state.host_terminal_theme, app.launch.theme);
    app.set_host_terminal_appearance(HostAppearance::Dark, true);
    assert_eq!(app.launch.appearance, Some(HostAppearance::Dark));
    let background = RgbColor { r: 1, g: 2, b: 3 };
    app.update_host_terminal_theme(DefaultColorKind::Background, background);
    assert_eq!(app.launch.theme.background, Some(background));
    assert!(app.launch.render_dirty.is_pending());
}

#[test]
fn theme_siblings_preserve_aliases_custom_names_and_authored_pair_overrides() {
    use maestro_daemon::{app::state::ThemeRuntimeConfig, terminal_theme::HostAppearance};
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
fn background_inference_selects_theme_until_explicit_appearance_is_reported() {
    use maestro_daemon::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
    let mut config = Config::default();
    config.theme.auto_switch = true;
    let mut app = app(&config);
    let white = RgbColor {
        r: 255,
        g: 255,
        b: 255,
    };
    app.update_host_terminal_theme(DefaultColorKind::Foreground, white);
    assert_eq!(app.state.host_terminal_appearance, None);
    app.update_host_terminal_theme(DefaultColorKind::Background, white);
    assert_eq!(
        app.state.host_terminal_appearance,
        Some(HostAppearance::Light)
    );
    assert_eq!(app.state.theme_name, "catppuccin-latte");
    assert!(!app.set_host_terminal_appearance(HostAppearance::Light, true));
    assert!(!app.set_host_terminal_appearance(HostAppearance::Dark, false));
    assert_eq!(
        app.state.host_terminal_appearance,
        Some(HostAppearance::Light)
    );
    assert!(app.state.host_terminal_appearance_explicit.0);
}

#[test]
fn daemon_app_new_falls_back_to_default_bounds_on_inverted_config() {
    let mut config = Config::default();
    config.ui.sidebar_min_width = 50;
    config.ui.sidebar_max_width = 30;
    let app = app(&config);
    assert_eq!(app.config.ui.sidebar_min_width, 18);
    assert_eq!(app.config.ui.sidebar_max_width, 36);
}
