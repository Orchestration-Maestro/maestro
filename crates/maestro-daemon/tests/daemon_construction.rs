//! Initial daemon presentation preferences come from the accepted configuration.
#![cfg(test)]
#![cfg(unix)]

#[path = "support/launch.rs"]
mod launch;

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
