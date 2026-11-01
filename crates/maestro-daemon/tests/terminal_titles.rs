//! Terminal title synchronization preserves public observations and sidebar damage.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support::launch;

use maestro_daemon::{api::event_hub::EventHub, app::App, settings::model::Config};
use maestro_workspace::layout::PaneId;
use std::{
    collections::HashSet,
    sync::{Arc, RwLock},
};

#[test]
fn setup_entry() {
    if maestro_process::pty::setup_entry().is_some() {
        std::process::exit(1);
    }
}

/// Constructs real native ownership without a shell emitting unsolicited titles.
fn fixture(args: &[String]) -> (App<fn() -> u128>, PaneId) {
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(args, manifests);
    let mut app = App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    );
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab(std::env::current_dir().unwrap(), None, |_, _| Ok(()))
        .unwrap();
    app.state.active = Some(0);
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let mut terminal = maestro_daemon::terminal::state::TerminalState::new(
        id.clone(),
        std::env::current_dir().unwrap(),
    );
    terminal.detected_agent = Some(maestro_detection::agents::Agent::Claude);
    terminal.state = maestro_detection::agents::AgentState::Working;
    app.state.terminals.insert(id.clone(), terminal);
    let mut config = app.launch.runtime_config(&app.config, pane, (24, 80));
    config.agent_detection = maestro_daemon::pane::runtime::AgentDetection::Disabled;
    let runtime = maestro_daemon::pane::runtime::PaneRuntime::spawn_shell_command(
        config,
        "read -r line",
        std::env::current_dir().unwrap(),
        &maestro_daemon::pane::launch_env::PaneLaunchEnv::default(),
    )
    .unwrap();
    app.terminal_runtimes.insert(id, runtime);
    (app, pane)
}

/// Feeds the actual terminal parser independently of native process output timing.
fn title(app: &App<fn() -> u128>, bytes: &[u8]) {
    let runtime = app.terminal_runtimes.values().next().unwrap();
    let _ = runtime.process_pty_bytes(
        app.state.topology.workspaces[0].tabs[0].root_pane,
        runtime.child_pid().unwrap(),
        bytes,
        || None,
    );
}

/// Counts the shared protocol events, not a separate title notification.
fn updates(app: &App<fn() -> u128>) -> usize {
    app.event_hub
        .events_after(0)
        .iter()
        .filter(|(_, event)| {
            matches!(
                event,
                maestro_protocol::events::EventData::PaneUpdated { .. }
            )
        })
        .count()
}

#[test]
fn daemon_sync_keeps_latest_raw_title_and_emits_only_for_stripped_changes() {
    let name = "daemon_sync_keeps_latest_raw_title_and_emits_only_for_stripped_changes";
    if maestro_test_process::run_alone(name).unwrap() {
        return;
    }
    maestro_test_process::TestProcess::named("terminal_titles::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_titles(args));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

/// Preserves every raw/stripped, revision, presentation and agent assertion.
async fn check_titles(args: &[String]) {
    let (mut app, pane) = fixture(args);
    let sources = HashSet::from([pane]);
    check_initial_title(&mut app, pane, &sources).await;
    check_spinner_title(&mut app, pane, &sources).await;
    title(&app, b"\x1b]0;Done reviewing\x07");
    assert!(app.sync_terminal_titles(&sources).await.stripped_changed);
    assert_eq!(updates(&app), 2);
    title(&app, b"\x1b]0;\x07");
    assert!(app.sync_terminal_titles(&sources).await.stripped_changed);
    let info = app.pane_info(0, pane).await.unwrap();
    assert_eq!(info.terminal_title, None);
    assert_eq!(info.terminal_title_stripped, None);
    assert_eq!(info.revision, 3);
    assert_eq!(updates(&app), 3);
    app.shutdown(&maestro_daemon::paths::DaemonPaths::new("/unused", None).unwrap())
        .await;
}

/// First publication exposes complete pane and agent presentation.
async fn check_initial_title(app: &mut App<fn() -> u128>, pane: PaneId, sources: &HashSet<PaneId>) {
    title(app, "\x1b]0;⠋ 修复🙂标题\x07".as_bytes());
    let changes = app.sync_terminal_titles(sources).await;
    assert!(changes.raw_changed && changes.stripped_changed);
    let info = app.pane_info(0, pane).await.unwrap();
    assert_eq!(info.terminal_title.as_deref(), Some("⠋ 修复🙂标题"));
    assert_eq!(info.terminal_title_stripped.as_deref(), Some("修复🙂标题"));
    assert_eq!(info.title, None);
    assert_eq!(
        info.agent_status,
        maestro_protocol::schema::AgentStatus::Working
    );
    assert_eq!(info.revision, 1);
    let agent = app.collect_agent_infos().await.pop().unwrap();
    assert_eq!(agent.terminal_title, info.terminal_title);
    assert_eq!(agent.terminal_title_stripped, info.terminal_title_stripped);
}

/// Spinner-only updates retain raw text without semantic publication.
async fn check_spinner_title(app: &mut App<fn() -> u128>, pane: PaneId, sources: &HashSet<PaneId>) {
    title(app, "\x1b]2;⠙ 修复🙂标题\x1b\\".as_bytes());
    let changes = app.sync_terminal_titles(sources).await;
    assert!(changes.raw_changed && !changes.stripped_changed);
    let info = app.pane_info(0, pane).await.unwrap();
    assert_eq!(info.terminal_title.as_deref(), Some("⠙ 修复🙂标题"));
    assert_eq!(info.terminal_title_stripped.as_deref(), Some("修复🙂标题"));
    assert_eq!(info.revision, 1);
    assert_eq!(updates(app), 1);
}

#[test]
fn daemon_syncing_pending_titles_preserves_sidebar_render_impact() {
    let name = "daemon_syncing_pending_titles_preserves_sidebar_render_impact";
    if maestro_test_process::run_alone(name).unwrap() {
        return;
    }
    maestro_test_process::TestProcess::named("terminal_titles::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(async {
                    let (mut app, pane) = fixture(args);
                    app.config.ui.sidebar.agents.rows = vec![vec![
                        maestro_daemon::settings::sidebar::AgentSidebarToken::TerminalTitleStripped,
                    ]];
                    title(&app, b"\x1b]0;building\x07");
                    let _ = app.launch.render_dirty.request_terminal_title(pane);
                    assert!(app.sync_pending_terminal_titles().await.stripped_changed);
                    assert!(app.launch.render_dirty.take().generic);
                    app.shutdown(
                        &maestro_daemon::paths::DaemonPaths::new("/unused", None).unwrap(),
                    )
                    .await;
                });
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

#[test]
fn daemon_sidebar_redraws_only_for_the_configured_title_form() {
    use maestro_daemon::{
        app::terminal_titles::TerminalTitleChanges, settings::sidebar::AgentSidebarToken,
    };
    let manifests = Arc::new(RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(&[], manifests);
    let mut app = App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    );
    app.config.ui.sidebar.agents.rows = vec![vec![AgentSidebarToken::Agent]];
    app.config.ui.sidebar.agents.rows_by_agent.insert(
        "claude".into(),
        vec![vec![AgentSidebarToken::TerminalTitleStripped]],
    );
    let spinner_only = TerminalTitleChanges {
        raw_changed: true,
        stripped_changed: false,
    };
    assert!(!app.terminal_title_sidebar_changed(&spinner_only));
    assert!(app.terminal_title_sidebar_changed(&TerminalTitleChanges {
        raw_changed: false,
        stripped_changed: true
    }));
    app.config.ui.sidebar.agents.rows_by_agent.insert(
        "claude".into(),
        vec![vec![AgentSidebarToken::TerminalTitle]],
    );
    assert!(app.terminal_title_sidebar_changed(&spinner_only));
}

#[test]
fn daemon_loop_consumes_titles_and_publishes_complete_pane_records() {
    let name = "daemon_loop_consumes_titles_and_publishes_complete_pane_records";
    if maestro_test_process::run_alone(name).unwrap() {
        return;
    }
    maestro_test_process::TestProcess::named("terminal_titles::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_loop_projection(args));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

/// A render wake reaches the loop and publishes the shared pane record before shutdown.
async fn check_loop_projection(args: &[String]) {
    let (mut app, pane) = fixture(args);
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state.terminals.get_mut(&id).unwrap().manual_label = Some("native".into());
    app.state.terminals.get_mut(&id).unwrap().cwd = "/retained-title".into();
    app.configure_window_title("{workspace}");
    assert_eq!(app.window_title().as_deref(), Some("retained-title"));
    title(&app, b"\x1b]0;ready\x07");
    let _ = app.launch.render_dirty.request_terminal_title(pane);
    app.launch.render_notify.notify_one();
    let hub = app.event_hub.clone();
    app.run(
        &maestro_daemon::paths::DaemonPaths::new("/unused", None).unwrap(),
        async {
            while hub.current_sequence() == 0 {
                tokio::task::yield_now().await;
            }
        },
    )
    .await;
    let info = hub
        .events_after(0)
        .into_iter()
        .find_map(|(_, event)| match event {
            maestro_protocol::events::EventData::PaneUpdated { pane } => Some(pane),
            _ => None,
        })
        .unwrap();
    assert_eq!(info.pane_id, "w1:p1");
    assert_eq!(info.terminal_id, id.to_string());
    assert_eq!(info.workspace_id, "w1");
    assert_eq!(info.tab_id, "w1:t1");
    assert!(info.focused);
    assert_eq!(
        info.cwd,
        Some(std::env::current_dir().unwrap().display().to_string())
    );
    assert_eq!(info.label.as_deref(), Some("native"));
    assert_eq!(info.agent.as_deref(), Some("claude"));
    assert_eq!(info.terminal_title.as_deref(), Some("ready"));
    assert_eq!(info.revision, 1);
    assert_eq!(info.scroll.unwrap().viewport_rows, 24);
}
