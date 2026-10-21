//! Public workspace identity forms preserve positional and stable numbering.
#![cfg(test)]

use maestro_daemon::{app::state::DaemonState, settings::model::Config};

#[test]
fn daemon_legacy_bare_tab_id_uses_tab_position_not_public_tab_number() {
    let mut state = DaemonState::new(
        &Config::default(),
        || 0,
        std::sync::Arc::new(std::sync::RwLock::new(
            maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
        )),
    );
    state
        .topology
        .new_with_tab("/workspace".into(), None, |_, _| Ok(()))
        .unwrap();
    for _ in 0..4 {
        state
            .topology
            .create_tab_with_runtime(0, |_, _| Ok(()))
            .unwrap()
            .unwrap();
    }
    assert!(state.topology.workspaces[0].close_tab(1));
    assert_eq!(state.topology.workspaces[0].tabs[2].number, 4);
    assert_eq!(state.topology.workspaces[0].tabs[3].number, 5);
    assert_eq!(state.parse_tab_id("w1:t4"), Some((0, 2)));
    assert_eq!(state.parse_tab_id("w1:4"), Some((0, 3)));
}

#[cfg(unix)]
#[path = "support/launch.rs"]
mod launch;

/// Builds the daemon with retained adapters but no native process.
#[cfg(unix)]
fn app(config: &Config) -> maestro_daemon::app::App<fn() -> u128> {
    let manifests = std::sync::Arc::new(std::sync::RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(&[], manifests);
    maestro_daemon::app::App::new(
        config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        maestro_daemon::api::event_hub::EventHub::default(),
    )
}

#[test]
#[cfg(unix)]
fn daemon_tab_info_number_uses_stable_public_tab_number() {
    let mut app = app(&Config::default());
    app.state
        .topology
        .new_with_tab("/workspace".into(), None, |_, _| Ok(()))
        .unwrap();
    for _ in 0..2 {
        app.state
            .topology
            .create_tab_with_runtime(0, |_, _| Ok(()))
            .unwrap()
            .unwrap();
    }
    assert!(app.state.topology.workspaces[0].close_tab(1));
    let tab = app.tab_info(0, 1).unwrap();
    assert_eq!(tab.tab_id, "w1:t3");
    assert_eq!(tab.number, 3);
    assert_eq!(tab.label, "2");
}

#[test]
#[cfg(unix)]
fn tab_projection_keeps_focus_and_agent_attention_independent_of_identity() {
    use maestro_daemon::terminal::state::TerminalState;
    use maestro_detection::agents::AgentState;
    use maestro_protocol::schema::AgentStatus;
    let mut app = app(&Config::default());
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/workspace".into(), None, |_, _| Ok(()))
        .unwrap();
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state.terminals.insert(
        terminal.clone(),
        TerminalState::new(terminal.clone(), "/workspace".into()),
    );
    app.state.active = Some(0);
    for (state, seen, expected) in [
        (AgentState::Unknown, true, AgentStatus::Unknown),
        (AgentState::Idle, true, AgentStatus::Idle),
        (AgentState::Working, true, AgentStatus::Working),
        (AgentState::Idle, false, AgentStatus::Done),
        (AgentState::Blocked, false, AgentStatus::Blocked),
    ] {
        app.state.terminals.get_mut(&terminal).unwrap().state = state;
        app.state.topology.workspaces[0]
            .pane_state_mut(pane)
            .unwrap()
            .seen = seen;
        let tab = app.tab_info(0, 0).unwrap();
        assert_eq!(tab.agent_status, expected);
        assert_eq!(tab.workspace_id, "w1");
        assert!(tab.focused);
        assert_eq!(tab.pane_count, 1);
    }
    app.state.active = None;
    assert!(!app.tab_info(0, 0).unwrap().focused);
    assert!(app.tab_info(0, 1).is_none());
    assert!(app.tab_info(1, 0).is_none());
}

/// Resolves creation through the real pending-name route without starting a child.
#[cfg(unix)]
async fn pending_cwd(
    policy: maestro_daemon::settings::model::NewTerminalCwdConfig,
    source: Option<&str>,
) -> std::path::PathBuf {
    let mut config = Config::default();
    config.ui.prompt_new_workspace_name.0 = true;
    config.terminal.new_cwd = policy;
    let mut app = app(&config);
    if let Some(cwd) = source {
        app.state
            .topology
            .new_with_tab(cwd.into(), None, |_, _| Ok(()))
            .unwrap();
        app.state.active = Some(0);
    }
    app.begin_workspace_create().await.unwrap();
    assert!(app.state.topology.workspaces.len() <= 1);
    app.state.pending_workspace_create_cwd.unwrap()
}

#[test]
#[cfg(unix)]
fn daemon_workspace_creation_in_navigate_mode_uses_selected_workspace_seed_cwd() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            use maestro_daemon::app::state::Mode;
            let mut config = Config::default();
            config.ui.prompt_new_workspace_name.0 = true;
            let mut app = app(&config);
            for cwd in ["/first", "/second"] {
                app.state
                    .topology
                    .new_with_tab(cwd.into(), None, |_, _| Ok(()))
                    .unwrap();
            }
            app.state.active = Some(0);
            app.state.selected = 1;
            app.state.mode = Mode::Navigate;
            app.begin_workspace_create().await.unwrap();
            assert_eq!(
                app.state.pending_workspace_create_cwd.as_ref(),
                Some(&app.state.topology.workspaces[1].identity_cwd)
            );
            assert_eq!(
                app.state.pending_workspace_create_cwd.as_deref(),
                Some(std::path::Path::new("/second"))
            );
        });
}

#[test]
#[cfg(unix)]
fn daemon_new_terminal_cwd_follow_uses_source_cwd() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let cwd = pending_cwd(
                maestro_daemon::settings::model::NewTerminalCwdConfig::Follow,
                Some("/source"),
            )
            .await;
            assert_eq!(cwd, std::path::Path::new("/source"));
        });
}

#[test]
#[cfg(unix)]
fn daemon_new_terminal_cwd_follow_without_source_uses_home() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let home = std::env::var_os("HOME").unwrap();
            let cwd = pending_cwd(
                maestro_daemon::settings::model::NewTerminalCwdConfig::Follow,
                None,
            )
            .await;
            assert_eq!(cwd, std::path::PathBuf::from(home));
        });
}

#[test]
#[cfg(unix)]
fn daemon_new_terminal_cwd_path_uses_configured_path() {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let cwd = pending_cwd(
                maestro_daemon::settings::model::NewTerminalCwdConfig::Path("/fixed".into()),
                Some("/source"),
            )
            .await;
            assert_eq!(cwd, std::path::Path::new("/fixed"));
        });
}
