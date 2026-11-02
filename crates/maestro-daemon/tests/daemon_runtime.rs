//! Public workspace identity forms preserve positional and stable numbering.
#![cfg(test)]
#![cfg(target_os = "linux")]

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
use crate::support::launch;

/// Builds the daemon with retained adapters but no native process.
#[cfg(unix)]
fn app(config: &Config) -> maestro_daemon::app::App<fn() -> u128> {
    app_with_args(config, &[])
}

/// Builds the daemon with the setup entry's own harness arguments.
#[cfg(unix)]
pub(crate) fn app_with_args(
    config: &Config,
    args: &[String],
) -> maestro_daemon::app::App<fn() -> u128> {
    let manifests = std::sync::Arc::new(std::sync::RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let (launch, receiver) = launch::launch_inputs(args, manifests);
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

/// Adds one viewport and its retained terminal metadata without starting a process.
#[cfg(unix)]
fn target_app() -> (
    maestro_daemon::app::App<fn() -> u128>,
    maestro_workspace::layout::PaneId,
) {
    let mut app = app(&Config::default());
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/target".into(), None, |_, _| Ok(()))
        .unwrap();
    attach_terminal(&mut app, pane);
    app.state.active = Some(0);
    (app, pane)
}

/// Supplies the terminal metadata normally installed by a successful native spawn.
#[cfg(unix)]
fn attach_terminal(
    app: &mut maestro_daemon::app::App<fn() -> u128>,
    pane: maestro_workspace::layout::PaneId,
) {
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state.terminals.insert(
        id.clone(),
        maestro_daemon::terminal::state::TerminalState::new(id, "/target".into()),
    );
}

#[test]
#[cfg(unix)]
fn daemon_terminal_and_agent_targets_treat_terminal_ids_differently() {
    let (app, pane) = target_app();
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .to_string();
    let resolved = app.resolve_terminal_target(&id).unwrap();
    assert_eq!(resolved.pane_id, pane);
    assert_eq!(resolved.terminal_id, id);
    assert!(matches!(
        app.resolve_agent_target(&id),
        Err(maestro_daemon::app::terminal_targets::TerminalTargetError::NotFound { .. })
    ));
}

#[test]
#[cfg(unix)]
fn daemon_agent_target_rejects_a_pane_that_only_has_a_launch_command() {
    let (mut app, pane) = target_app();
    app.state.terminals.values_mut().next().unwrap().launch_argv =
        Some(vec!["just".into(), "dev".into()]);
    let id = app.state.public_pane_id(0, pane).unwrap();
    assert!(app.resolve_terminal_target(&id).is_ok());
    assert!(matches!(
        app.resolve_agent_target(&id),
        Err(maestro_daemon::app::terminal_targets::TerminalTargetError::NotFound { .. })
    ));
}

#[test]
#[cfg(unix)]
fn daemon_terminal_target_resolves_pane_id_for_an_agent() {
    let (mut app, pane) = target_app();
    detect_agent(&mut app, pane);
    let id = app.state.public_pane_id(0, pane).unwrap();
    let resolved = app.resolve_terminal_target(&id).unwrap();
    assert_eq!(resolved.pane_id, pane);
    assert_eq!(
        resolved.terminal_id,
        app.state.topology.workspaces[0]
            .terminal_id(pane)
            .unwrap()
            .to_string()
    );
}

#[test]
#[cfg(unix)]
fn daemon_terminal_target_resolves_unique_agent_name() {
    let (mut app, pane) = target_app();
    app.state
        .terminals
        .values_mut()
        .next()
        .unwrap()
        .set_agent_name("reviewer".into());
    let resolved = app.resolve_terminal_target("reviewer").unwrap();
    assert_eq!(resolved.pane_id, pane);
    assert_eq!(
        resolved.terminal_id,
        app.state.topology.workspaces[0]
            .terminal_id(pane)
            .unwrap()
            .to_string()
    );
}

#[test]
#[cfg(unix)]
fn daemon_agent_target_treats_legacy_pane_syntax_as_a_name() {
    let (mut app, pane) = target_app();
    detect_agent(&mut app, pane);
    let terminal = app.state.terminals.values_mut().next().unwrap();
    terminal.set_agent_name("p_1".into());
    let resolved = app.resolve_agent_target("p_1").unwrap();
    assert_eq!(resolved.pane_id, pane);
    assert_eq!(
        resolved.terminal_id,
        app.state.topology.workspaces[0]
            .terminal_id(pane)
            .unwrap()
            .to_string()
    );
}

#[test]
#[cfg(unix)]
fn daemon_terminal_target_reports_missing_target() {
    let (app, _) = target_app();
    assert_eq!(
        app.resolve_terminal_target("missing-agent").unwrap_err(),
        maestro_daemon::app::terminal_targets::TerminalTargetError::NotFound {
            target: "missing-agent".into()
        }
    );
}

#[test]
#[cfg(unix)]
fn daemon_terminal_target_reports_ambiguous_duplicate_agent_name() {
    use maestro_daemon::app::terminal_targets::TerminalTargetError;
    let (mut app, first) = target_app();
    let (second, ()) = app
        .state
        .topology
        .split_pane_with_runtime(
            0,
            maestro_workspace::layout::SplitRequest {
                target: first,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: None,
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap();
    attach_terminal(&mut app, second);
    for terminal in app.state.terminals.values_mut() {
        terminal.set_agent_name("worker".into());
    }
    let TerminalTargetError::Ambiguous { target, candidates } =
        app.resolve_terminal_target("worker").unwrap_err()
    else {
        panic!("expected ambiguity")
    };
    assert_eq!(target, "worker");
    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate.terminal_id.starts_with("term_")
                && candidate.pane_id.starts_with("w1:")
                && candidate.workspace_id == "w1"
                && candidate.cwd.is_some())
    );
}

/// Admits the same process identity observation consumed by live panes.
#[cfg(unix)]
fn detect_agent(
    app: &mut maestro_daemon::app::App<fn() -> u128>,
    pane: maestro_workspace::layout::PaneId,
) {
    app.state
        .handle_app_event(maestro_daemon::events::AppEvent::AgentProcessDetected {
            pane_id: pane,
            agent: maestro_detection::agents::Agent::Claude,
            observed_at: std::time::Instant::now(),
        });
}

#[test]
#[cfg(unix)]
fn terminal_target_precedence_and_agent_labels_do_not_admit_historical_pane_aliases() {
    use maestro_daemon::app::terminal_targets::TerminalTargetError;
    let (mut app, pane) = target_app();
    detect_agent(&mut app, pane);
    let id = app.state.public_pane_id(0, pane).unwrap();
    assert_eq!(app.resolve_agent_target(&id).unwrap().pane_id, pane);
    assert_eq!(app.resolve_terminal_target("claude").unwrap().pane_id, pane);
    assert!(matches!(
        app.resolve_agent_target("claude"),
        Err(TerminalTargetError::NotFound { .. })
    ));
    for form in ["p_1", "p_w1_1", "w1-1", "w1:p0", "w1:pz", "missing"] {
        assert!(
            matches!(
                app.resolve_terminal_target(form),
                Err(TerminalTargetError::NotFound { .. })
            ),
            "{form}"
        );
    }
    app.state
        .public_pane_id_aliases
        .insert("stale-alias".into(), pane);
    assert!(matches!(
        app.resolve_terminal_target("stale-alias"),
        Err(TerminalTargetError::NotFound { .. })
    ));
}

#[test]
#[cfg(unix)]
fn terminal_identity_precedes_a_colliding_name_and_agent_names_remain_ambiguous() {
    use maestro_daemon::app::terminal_targets::TerminalTargetError;
    let (mut app, pane) = target_app();
    let (other, ()) = app
        .state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    attach_terminal(&mut app, other);
    let terminal_id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .to_string();
    let other_id = app.state.topology.workspaces[0]
        .terminal_id(other)
        .unwrap()
        .clone();
    app.state
        .terminals
        .get_mut(&other_id)
        .unwrap()
        .set_agent_name(terminal_id.clone());
    assert_eq!(
        app.resolve_terminal_target(&terminal_id).unwrap().pane_id,
        pane
    );
    assert_eq!(
        app.resolve_agent_target(&terminal_id).unwrap().pane_id,
        other
    );
    for terminal in app.state.terminals.values_mut() {
        terminal.set_agent_name("duplicate".into());
    }
    let TerminalTargetError::Ambiguous { candidates, .. } =
        app.resolve_agent_target("duplicate").unwrap_err()
    else {
        panic!("expected ambiguity")
    };
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0].tab_id, "w1:t1");
    assert_eq!(candidates[1].tab_id, "w1:t2");
}

/// Builds the screen-state observation used by native fallback detection.
#[cfg(unix)]
fn state_event(
    pane_id: maestro_workspace::layout::PaneId,
    state: maestro_detection::agents::AgentState,
) -> maestro_daemon::events::AppEvent {
    maestro_daemon::events::AppEvent::StateChanged {
        pane_id,
        state,
        agent: Some(maestro_detection::agents::Agent::Claude),
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: std::time::Instant::now(),
    }
}

#[test]
#[cfg(unix)]
fn daemon_full_internal_event_queue_eventually_applies_working_to_idle_transition() {
    use maestro_detection::agents::AgentState;
    let (mut app, pane) = target_app();
    app.handle_internal_event(state_event(pane, AgentState::Working));
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    assert_eq!(app.state.terminals[&id].state, AgentState::Working);
    let sender = app.launch.events.clone();
    for _ in 0..sender.max_capacity() {
        sender
            .try_send(maestro_daemon::events::AppEvent::TerminalBell {
                pane_id: pane,
                count: 1,
            })
            .unwrap();
    }
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let mut send = std::pin::pin!(sender.send(state_event(pane, AgentState::Idle)));
            let mut context = std::task::Context::from_waker(std::task::Waker::noop());
            assert!(std::future::Future::poll(send.as_mut(), &mut context).is_pending());
            app.drain_internal_events();
            send.await.unwrap();
            for _ in 0..(sender.max_capacity() / 64 + 2) {
                app.drain_internal_events();
            }
        });
    assert_eq!(app.state.terminals[&id].state, AgentState::Idle);
}

#[test]
#[cfg(unix)]
fn setup_entry() {
    if maestro_process::pty::setup_entry().is_some() {
        std::process::exit(1);
    }
}

#[test]
#[cfg(unix)]
fn shutdown_drains_pending_exit_hooks_before_runtime_release() {
    let name = "shutdown_drains_pending_exit_hooks_before_runtime_release";
    if maestro_test_process::run_alone(name).unwrap() {
        return;
    }
    maestro_test_process::TestProcess::named("daemon_runtime::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_shutdown(args));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

/// Holds a real child until shutdown while the critical event queue is full.
#[cfg(unix)]
async fn check_shutdown(args: &[String]) {
    use maestro_daemon::{events::AppEvent, paths::DaemonPaths};
    let (mut app, pane) = live_shutdown_app(args).await;
    app.state
        .terminals
        .values_mut()
        .next()
        .unwrap()
        .respawn_shell_on_exit = true;
    app.handle_internal_event(state_event(
        pane,
        maestro_detection::agents::AgentState::Working,
    ));
    let before = app.event_hub.current_sequence();
    for _ in 0..app.launch.events.max_capacity() {
        app.launch
            .events
            .try_send(AppEvent::TerminalBell {
                pane_id: pane,
                count: 1,
            })
            .unwrap();
    }
    assert_eq!(app.launch.events.capacity(), 0);
    let runtime = app.terminal_runtimes.values().next().unwrap();
    let pid = runtime.child_pid().unwrap();
    runtime.send_bytes(b"\n".to_vec()).await.unwrap();
    while maestro_process::child_process::process_exists(pid) {
        tokio::task::yield_now().await;
    }
    app.shutdown(&DaemonPaths::new("/unused", None).unwrap())
        .await;
    let events = app.event_hub.events_after(before);
    let release = events
        .iter()
        .position(|(_, event)| {
            matches!(
                event,
                maestro_protocol::events::EventData::PaneAgentDetected { released: true, .. }
            )
        })
        .unwrap();
    let exit = events
        .iter()
        .position(|(_, event)| {
            matches!(
                event,
                maestro_protocol::events::EventData::PaneExited { .. }
            )
        })
        .unwrap();
    assert!(release < exit);
    assert!(app.terminal_runtimes.is_empty());
    assert!(app.state.topology.workspaces.is_empty());
}

/// Starts a checked child and waits for its already-ingested readiness observation.
#[cfg(unix)]
async fn live_shutdown_app(
    args: &[String],
) -> (
    maestro_daemon::app::App<fn() -> u128>,
    maestro_workspace::layout::PaneId,
) {
    use maestro_daemon::{
        events::AppEvent,
        pane::{
            launch_env::PaneLaunchEnv,
            runtime::{AgentDetection, PaneRuntime},
        },
    };
    let (mut app, pane) = target_app();
    let (launch, receiver) =
        launch::launch_inputs(args, std::sync::Arc::clone(&app.state.manifests));
    app.launch = launch;
    app.event_rx = receiver;
    let mut config = app.launch.runtime_config(&app.config, pane, (24, 80));
    config.agent_detection = AgentDetection::Disabled;
    let runtime = PaneRuntime::spawn_shell_command(
        config,
        "printf '\\033]7;file:///tmp\\033\\\\'; read -r line",
        std::env::current_dir().unwrap(),
        &PaneLaunchEnv::default(),
    )
    .unwrap();
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.terminal_runtimes.insert(id, runtime);
    loop {
        if matches!(
            app.event_rx.recv().await.unwrap(),
            AppEvent::TerminalCwdReported { .. }
        ) {
            break;
        }
    }
    (app, pane)
}

#[test]
fn daemon_construction_initializes_supplied_manifest_summaries() {
    let manifests = std::sync::Arc::new(std::sync::RwLock::new(
        maestro_detection::manifest::ManifestCache::new(None, None).unwrap(),
    ));
    let expected = manifests.read().unwrap().manifest_summaries();
    assert!(!expected.is_empty());
    let state = DaemonState::new(&Config::default(), || 0, manifests);
    assert_eq!(state.agent_manifest_summaries, expected);
}

/// Prepares a home directory, then runs this test again with it as `HOME`.
///
/// Returns the directory in that child and `None` in the parent.
#[cfg(unix)]
fn isolated_home(
    test: &str,
    prepare: impl FnOnce(&std::path::Path) -> std::path::PathBuf,
) -> Option<std::path::PathBuf> {
    const CHILD: &str = "MAESTRO_DAEMON_HOME_CHILD";
    if std::env::var_os(CHILD).is_some() {
        return std::env::var_os("HOME").map(std::path::PathBuf::from);
    }
    let owner = maestro_test_directory::Scratch::new().unwrap();
    let home = prepare(owner.path());
    let output = maestro_test_process::TestProcess::current(test)
        .unwrap()
        .env("HOME", &home)
        .env(CHILD, "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "child {test} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    None
}

/// Creates the home directory whose spelling text cannot hold, below `root`.
#[cfg(unix)]
fn unspellable_home(root: &std::path::Path) -> std::path::PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let home = root.join(<std::ffi::OsStr as OsStrExt>::from_bytes(b"home-\xff"));
    std::fs::create_dir_all(home.join("child")).unwrap();
    home
}

/// Creates the default workspace through the prompt-free creation entry.
#[cfg(unix)]
fn default_workspace_cwd(policy: &str) -> std::path::PathBuf {
    let mut config = Config::default();
    config.terminal.default_shell = "/bin/sh".into();
    config.terminal.new_cwd =
        maestro_daemon::settings::model::NewTerminalCwdConfig::Path(policy.into());
    let args = maestro_test_process::TestProcess::named("daemon_runtime::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| Ok::<_, std::io::Error>(args.to_vec()))
        .unwrap();
    let mut app = app_with_args(&config, &args);
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            assert!(app.ensure_default_workspace());
            let cwd = app.state.topology.workspaces[0].identity_cwd.clone();
            for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
                runtime.shutdown().await;
            }
            cwd
        })
}

/// A configured `~` reaches the directory its written parent segment names.
#[cfg(unix)]
#[test]
fn daemon_new_terminal_cwd_follows_the_link_above_a_tilde_path() {
    let Some(home) = isolated_home(
        "daemon_new_terminal_cwd_follows_the_link_above_a_tilde_path",
        |root| {
            std::fs::create_dir_all(root.join("elsewhere/child")).unwrap();
            std::fs::create_dir(root.join("elsewhere/f")).unwrap();
            std::fs::create_dir(root.join("f")).unwrap();
            std::os::unix::fs::symlink(root.join("elsewhere/child"), root.join("link")).unwrap();
            root.to_path_buf()
        },
    ) else {
        return;
    };
    let cwd = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(pending_cwd(
            maestro_daemon::settings::model::NewTerminalCwdConfig::Path("~/link/../f".into()),
            None,
        ));
    assert_eq!(
        std::fs::canonicalize(&cwd).unwrap(),
        std::fs::canonicalize(home.join("elsewhere/f")).unwrap()
    );
}

/// A `~` path's written rest joins below the home directory as written, so an absolute rest
/// stays below it, and `~name` is not a home form.
#[cfg(unix)]
#[test]
fn daemon_tilde_paths_join_their_rest_natively() {
    let Some(home) = isolated_home("daemon_tilde_paths_join_their_rest_natively", |root| {
        root.to_path_buf()
    }) else {
        return;
    };
    let home = home.to_string_lossy().into_owned();
    let cases = [
        ("~", home.clone()),
        ("~/", format!("{home}/")),
        ("~//note", format!("{home}//note")),
        ("~/a/../f", format!("{home}/a/../f")),
        ("~//x", format!("{home}//x")),
        ("~x", "~x".to_owned()),
    ];
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for (policy, expected) in cases {
                let cwd = pending_cwd(
                    maestro_daemon::settings::model::NewTerminalCwdConfig::Path(policy.into()),
                    None,
                )
                .await;
                assert_eq!(cwd.to_string_lossy(), expected, "{policy:?}");
            }
        });
}

/// A home directory that text cannot hold stays the created workspace directory.
#[cfg(unix)]
#[test]
fn daemon_workspace_creation_keeps_native_home_bytes() {
    let Some(home) = isolated_home(
        "daemon_workspace_creation_keeps_native_home_bytes",
        unspellable_home,
    ) else {
        return;
    };
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            for (policy, expected) in [("~", home.clone()), ("~/child", home.join("child"))] {
                let cwd = pending_cwd(
                    maestro_daemon::settings::model::NewTerminalCwdConfig::Path(policy.into()),
                    None,
                )
                .await;
                assert_eq!(cwd, expected, "{policy:?}");
            }
        });
}

/// A home directory that text cannot hold stays the default workspace directory.
#[cfg(unix)]
#[test]
fn daemon_default_workspace_keeps_native_home_bytes() {
    let Some(home) = isolated_home(
        "daemon_default_workspace_keeps_native_home_bytes",
        unspellable_home,
    ) else {
        return;
    };
    assert_eq!(default_workspace_cwd("~/child"), home.join("child"));
}
