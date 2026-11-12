//! Initial daemon presentation preferences come from the accepted configuration.

use crate::state::test_support::api::daemon_with;
use crate::state::test_support::spawning;
use crate::{
    service::event_hub::EventHub,
    settings::model::{AgentPanelSortConfig, Config},
    state::{App, model::AgentPanelSort},
};
use std::sync::{Arc, RwLock};

#[test]
fn construction_uses_configured_agent_panel_sort() {
    let mut config = Config::default();
    config.ui.agent_panel_sort = AgentPanelSortConfig::Priority;
    assert_eq!(
        daemon_with(&config).unwrap().state.agent_panel_sort,
        AgentPanelSort::Priority
    );
}
#[test]
fn construction_uses_configured_sidebar_state() {
    let mut config = Config::default();
    config.ui.sidebar_start_collapsed.0 = true;
    assert!(daemon_with(&config).unwrap().state.sidebar_collapsed.0);
}
#[test]
fn construction_uses_redraw_on_focus_gained_config() {
    let mut config = Config::default();
    config.ui.redraw_on_focus_gained.0 = false;
    assert!(!daemon_with(&config).unwrap().state.redraw_on_focus_gained.0);
}
#[test]
fn construction_uses_workspace_name_prompt_config() {
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    assert!(
        daemon_with(&config)
            .unwrap()
            .state
            .prompt_new_workspace_name
            .0
    );
}
#[test]
fn pending_workspace_creation_blocks_default_until_cancelled() {
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    let mut app = daemon_with(&config).unwrap();
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
    use crate::{state::model::Mode, terminal::model::TerminalState};
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    let mut app = daemon_with(&config).unwrap();
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
    use crate::settings::model::NewTerminalCwdConfig;
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    let mut app = daemon_with(&config).unwrap();
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
    let mut app = daemon_with(&config).unwrap();
    assert!(!app.state.theme_runtime.auto_switch);
    assert_eq!(app.state.theme_name, "tokyo-night");
    assert!(!app.set_host_terminal_appearance(crate::terminal_theme::HostAppearance::Light, true,));
    assert_eq!(app.state.theme_name, "tokyo-night");
}
#[test]
fn construction_selects_sibling_theme_for_explicit_host_appearance() {
    use crate::terminal_theme::HostAppearance;
    let mut config = Config::default();
    config.theme.name = Some("tokyo-night".into());
    config.theme.auto_switch = true;
    let mut app = daemon_with(&config).unwrap();
    assert_eq!(app.state.theme_name, "tokyo-night");
    assert!(app.set_host_terminal_appearance(HostAppearance::Light, true));
    assert_eq!(app.state.theme_name, "tokyo-night-day");
    assert!(!app.set_host_terminal_appearance(HostAppearance::Light, true));
    assert!(app.set_host_terminal_appearance(HostAppearance::Dark, true));
    assert_eq!(app.state.theme_name, "tokyo-night");
}
#[test]
fn construction_retains_custom_overrides_when_selecting_active_theme() {
    use crate::{settings::appearance::CustomThemeColors, terminal_theme::HostAppearance};
    let mut config = Config::default();
    config.theme.name = Some("gruvbox".into());
    config.theme.auto_switch = true;
    config.theme.custom = Some(CustomThemeColors {
        accent: Some("#010203".into()),
        ..CustomThemeColors::default()
    });
    config.ui.accent = "red".into();
    let mut app = daemon_with(&config).unwrap();
    assert!(app.set_host_terminal_appearance(HostAppearance::Light, true));
    assert_eq!(app.state.theme_name, "gruvbox-light");
    assert_eq!(app.state.theme_runtime.custom, config.theme.custom);
    assert!(app.state.theme_runtime.legacy_accent.is_none());
}
#[test]
fn daemon_inferred_background_appearance_does_not_override_explicit_report() {
    use crate::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
    let mut config = Config::default();
    config.theme.name = Some("catppuccin".into());
    config.theme.auto_switch = true;
    let mut app = daemon_with(&config).unwrap();
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
    use crate::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
    let mut config = Config::default();
    config.theme.auto_switch = true;
    let manifests = Arc::new(RwLock::new(
        maestro_detection::definitions::ManifestCache::new(None, None).unwrap(),
    ));
    let (mut launch, receiver) = spawning::launch_inputs(&[], manifests).unwrap();
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
fn background_inference_selects_theme_until_explicit_appearance_is_reported() {
    use crate::terminal_theme::{DefaultColorKind, HostAppearance, RgbColor};
    let mut config = Config::default();
    config.theme.auto_switch = true;
    let mut app = daemon_with(&config).unwrap();
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
