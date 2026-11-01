//! Internal event delivery uses live retained terminal runtimes.
#![cfg(test)]
#![cfg(target_os = "linux")]

use crate::support::{self, launch::launch_inputs};

use maestro_daemon::{
    app::App,
    events::AppEvent,
    pane::{
        launch_env::PaneLaunchEnv,
        runtime::{AgentDetection, PaneRuntime},
    },
    settings::model::Config,
};
use maestro_detection::agents::{Agent, AgentState};
use maestro_test_directory::temporary_root;
use maestro_test_process::{TestProcess, run_alone};
use maestro_workspace::layout::PaneId;

/// Daemon owner driven by a deterministic public-identity clock.
type TestApp = App<fn() -> u128>;
use std::{sync::Arc, time::Instant};
use tokio::sync::mpsc;

#[test]
fn setup_entry() {
    if maestro_process::pty::setup_entry().is_some() {
        std::process::exit(1);
    }
}

/// Runs one real pane transaction without sharing its process session with other tests.
fn with_live_root(name: &str, delayed: bool) {
    if run_alone(name).unwrap() {
        return;
    }
    TestProcess::named("daemon_events::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_live_context(args, delayed));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

/// Retains stale workspace metadata while the process reports its actual directory.
async fn check_live_context(args: &[String], delayed: bool) {
    let directory = context_directories();
    let root = directory.path().to_path_buf();
    let live = root.join("__maestro_projects__");
    let mut app = context_app(root.join("__maestro_original__"), delayed);
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let (launch, event_rx) = launch_inputs(args, Arc::clone(&app.state.manifests));
    app.launch = launch;
    app.event_rx = event_rx;
    let mut config = app.launch.runtime_config(&app.config, pane, (24, 80));
    config.agent_detection = AgentDetection::Disabled;
    let command = format!(
        "printf '\\033]7;file://{}\\033\\\\'; read -r line",
        live.display()
    );
    let runtime =
        PaneRuntime::spawn_shell_command(config, &command, live.clone(), &PaneLaunchEnv::default())
            .unwrap();
    loop {
        if let Some(AppEvent::TerminalCwdReported { cwd, .. }) = app.event_rx.recv().await {
            assert_eq!(cwd, live);
            break;
        }
    }
    assert_eq!(runtime.cwd(), Some(live));
    app.terminal_runtimes.insert(terminal, runtime);
    for state in [AgentState::Working, AgentState::Idle] {
        app.handle_internal_event(AppEvent::StateChanged {
            pane_id: pane,
            agent: Some(Agent::Codex),
            state,
            visible_blocker: false,
            visible_working: false,
            process_exited: false,
            observed_at: Instant::now(),
        });
    }
    if delayed {
        assert_delayed_context(&mut app);
    }
    assert_eq!(
        app.state.toast.as_ref().map(|toast| toast.context.as_str()),
        Some("__maestro_projects__ · 1")
    );
    for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
        runtime.shutdown().await;
    }
    std::fs::remove_dir_all(root).unwrap();
}

/// Checks both notification destinations after the exact scheduled deadline.
fn assert_delayed_context(app: &mut TestApp) {
    assert!(app.state.toast.is_none());
    let deadline = app
        .state
        .next_pending_agent_notification_deadline()
        .unwrap();
    let deliveries = app.drain_due_agent_notifications(deadline);
    assert_eq!(deliveries.len(), 1);
    assert_eq!(
        deliveries[0].toast.as_ref().unwrap().context,
        "__maestro_projects__ · 1"
    );
    assert_eq!(
        deliveries[0].client_notification.as_ref().unwrap().context,
        "__maestro_projects__ · 1"
    );
}

/// Creates distinct admitted repositories for the original and observed directories.
fn context_directories() -> maestro_test_directory::Scratch {
    let root = maestro_test_directory::Scratch::new().unwrap();
    for name in ["__maestro_original__", "__maestro_projects__"] {
        let path = root.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(path)
                .status()
                .unwrap()
                .success()
        );
    }
    root
}

/// Builds the background workspace without admitting the process's new directory.
fn context_app(directory: std::path::PathBuf, delayed: bool) -> TestApp {
    let config = Config::default();
    let (launch, event_rx) = launch_inputs(&[], support::manifest_cache());
    let mut app = App::new(
        &config,
        (|| 0) as fn() -> u128,
        launch,
        event_rx,
        maestro_daemon::api::event_hub::EventHub::default(),
    );
    let state = &mut app.state;
    state
        .topology
        .new_with_tab(directory, None, |_, _| Ok(()))
        .unwrap();
    support::populate_terminals(state);
    state.active = Some(0);
    support::assert_identity_invariants(state);
    state.active = None;
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    state.toast_config.delay_seconds = u64::from(delayed);
    state.sound.enabled = false;
    app
}

#[test]
fn daemon_workspace_toast_context_uses_live_root_runtime_cwd_label() {
    with_live_root(
        "daemon_workspace_toast_context_uses_live_root_runtime_cwd_label",
        false,
    );
}

#[test]
fn daemon_delayed_workspace_toast_context_uses_live_root_runtime_cwd_label() {
    with_live_root(
        "daemon_delayed_workspace_toast_context_uses_live_root_runtime_cwd_label",
        true,
    );
}

/// Adds one temporary overlay split through the topology's consuming callback.
fn overlay_app(previous_zoomed: bool) -> (TestApp, PaneId, PaneId) {
    let mut app = context_app("/overlay".into(), false);
    app.state.active = Some(0);
    app.state.mode = maestro_daemon::app::state::Mode::Terminal;
    let previous_focus = app.state.topology.workspaces[0].tabs[0].root_pane;
    let pane = app
        .state
        .topology
        .split_pane_with_runtime(
            0,
            maestro_workspace::layout::SplitRequest {
                target: previous_focus,
                direction: maestro_protocol::schema::SplitDirection::Right,
                ratio: Some(0.5),
                focus_new_pane: true,
            },
            |_, _| Ok(()),
        )
        .unwrap()
        .unwrap()
        .0;
    support::populate_terminals(&mut app.state);
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    app.overlay_panes.insert(
        pane,
        maestro_daemon::app::OverlayPaneState {
            ws_idx: 0,
            tab_idx: 0,
            previous_focus,
            previous_zoomed,
            temp_files: Vec::new(),
        },
    );
    (app, pane, previous_focus)
}

#[test]
fn daemon_overlay_exit_preserves_focus_changed_before_exit() {
    let (mut app, pane, previous_focus) = overlay_app(true);
    app.state
        .topology
        .create_tab_with_runtime(0, |_, _| Ok(()))
        .unwrap()
        .unwrap();
    let new_tab = 1;
    app.state.topology.workspaces[0].switch_tab(new_tab);
    support::populate_terminals(&mut app.state);
    app.handle_internal_event(AppEvent::PaneDied { pane_id: pane });
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.active_tab, new_tab);
    assert_eq!(workspace.tabs[0].layout.focused(), previous_focus);
    assert!(workspace.tabs[0].zoomed);
    assert!(app.overlay_panes.is_empty());
}

#[test]
fn daemon_idle_agent_exit_emits_release_event_without_a_state_change() {
    for agent_name in [None, Some("reviewer")] {
        let mut app = context_app("/idle-agent-exit".into(), false);
        let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
        let terminal_id = app.state.topology.workspaces[0]
            .terminal_id(pane)
            .unwrap()
            .clone();
        app.state.handle_app_event(AppEvent::StateChanged {
            pane_id: pane,
            agent: Some(Agent::Pi),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
            process_exited: false,
            observed_at: Instant::now(),
        });
        let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
        if let Some(name) = agent_name {
            terminal.set_agent_name(name.into());
        }
        app.handle_internal_event(AppEvent::StateChanged {
            pane_id: pane,
            agent: Some(Agent::Pi),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
            process_exited: true,
            observed_at: Instant::now(),
        });
        assert!(app.state.terminals[&terminal_id].agent_name.is_none());
        assert!(
            app.event_hub
                .events_after(0)
                .iter()
                .any(|(_, event)| matches!(
                    event,
                    maestro_protocol::events::EventData::PaneAgentDetected {
                        released: true,
                        final_status: Some(maestro_protocol::schema::AgentStatus::Idle),
                        ..
                    }
                ))
        );
    }
}

#[test]
fn daemon_process_exit_releases_a_newer_hook_owned_agent() {
    let mut app = context_app("/stale-agent-exit".into(), false);
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let observed_at = Instant::now();
    app.state.handle_app_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Codex),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at,
    });
    let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
    terminal
        .set_hook_authority_at(
            maestro_daemon::terminal::state::HookReport {
                source: "maestro:codex".into(),
                agent_label: "codex".into(),
                state: AgentState::Working,
                message: None,
                session_ref: None,
                seq: Some(1),
            },
            observed_at + std::time::Duration::from_secs(1),
        )
        .unwrap();
    terminal.set_agent_name("reviewer".into());
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Codex),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: true,
        observed_at,
    });
    let terminal = &app.state.terminals[&terminal_id];
    assert_eq!(terminal.state, AgentState::Idle);
    assert!(terminal.agent_name.is_none());
    assert!(
        app.event_hub
            .events_after(0)
            .iter()
            .any(|(_, event)| matches!(
                event,
                maestro_protocol::events::EventData::PaneAgentDetected { released: true, .. }
            ))
    );
}

#[test]
fn daemon_overlay_exit_preserves_same_tab_focus_changed_before_exit() {
    let (mut app, pane, previous_focus) = overlay_app(false);
    app.state.topology.workspaces[0].tabs[0]
        .layout
        .focus_pane(previous_focus);
    app.handle_internal_event(AppEvent::PaneDied { pane_id: pane });
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.active_tab, 0);
    assert_eq!(workspace.tabs[0].layout.focused(), previous_focus);
    assert!(workspace.tabs[0].zoomed);
    assert!(app.overlay_panes.is_empty());
}

#[test]
fn daemon_overlay_exit_restores_previous_focus_when_overlay_still_focused() {
    let (mut app, pane, previous_focus) = overlay_app(false);
    let directory = maestro_test_directory::Scratch::new().unwrap();
    let temporary_file = directory.path().join("overlay-owned");
    std::fs::write(&temporary_file, "overlay-owned").unwrap();
    app.overlay_panes
        .get_mut(&pane)
        .unwrap()
        .temp_files
        .push(temporary_file.clone());
    app.handle_internal_event(AppEvent::PaneDied { pane_id: pane });
    assert!(!temporary_file.exists());
    let workspace = &app.state.topology.workspaces[0];
    assert_eq!(workspace.active_tab, 0);
    assert_eq!(workspace.tabs[0].layout.focused(), previous_focus);
    assert!(!workspace.tabs[0].zoomed);
    assert!(app.overlay_panes.is_empty());
}

#[test]
fn pane_death_publishes_completion_before_exit_and_identity_removal() {
    use maestro_protocol::{events::EventData, schema::AgentStatus};
    let mut app = context_app("/completing-pane".into(), false);
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let public_id = app.state.public_pane_id(0, pane).unwrap();
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Codex),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: Instant::now(),
    });
    let sequence = app.event_hub.current_sequence();
    app.handle_internal_event(AppEvent::PaneDied { pane_id: pane });
    let events = app.event_hub.events_after(sequence);
    let release = events.iter().position(|(_, event)| matches!(event,
        EventData::PaneAgentDetected { pane_id, released: true, final_status: Some(AgentStatus::Done), .. } if *pane_id == public_id
    )).unwrap();
    let exit = events
        .iter()
        .position(|(_, event)| {
            matches!(event,
                EventData::PaneExited { pane_id, .. } if *pane_id == public_id
            )
        })
        .unwrap();
    assert!(release < exit);
    assert!(app.state.find_pane(pane).is_none());
    assert!(app.state.toast.as_ref().unwrap().title.contains("finished"));
}

#[test]
fn daemon_pane_died_respawns_shell_and_clears_restored_agent_session() {
    if run_alone("daemon_pane_died_respawns_shell_and_clears_restored_agent_session").unwrap() {
        return;
    }
    TestProcess::named("daemon_events::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_respawn(args));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

/// A launch-command exit keeps its pane but replaces its process and clears agent ownership.
async fn check_respawn(args: &[String]) {
    let mut app = context_app(temporary_root(), false);
    let (launch, event_rx) = launch_inputs(args, Arc::clone(&app.state.manifests));
    app.launch = launch;
    app.event_rx = event_rx;
    app.config.terminal.default_shell = "/bin/sh".into();
    app.config.terminal.shell_mode = maestro_daemon::settings::model::ShellModeConfig::NonLogin;
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let terminal = app.state.terminals.get_mut(&terminal_id).unwrap();
    terminal.respawn_shell_on_exit = true;
    terminal.set_agent_name("codex".into());
    terminal.set_persisted_agent_session(maestro_daemon::agent_resume::PersistedAgentSession {
        source: "maestro:codex".into(),
        agent: "codex".into(),
        session_ref: maestro_daemon::agent_resume::AgentSessionRef::id("codex-session").unwrap(),
    });
    app.handle_internal_event(AppEvent::PaneDied { pane_id: pane });
    assert!(app.state.find_pane(pane).is_some());
    let terminal = &app.state.terminals[&terminal_id];
    assert!(!terminal.respawn_shell_on_exit);
    assert!(terminal.persisted_agent_session.is_none());
    assert!(terminal.agent_name.is_none());
    assert_shell_identity(&app, &terminal_id).await;
    assert_eq!(app.terminal_runtimes.len(), 1);
    assert!(
        app.terminal_runtimes
            .get(&terminal_id)
            .unwrap()
            .child_pid()
            .is_some()
    );
    for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
        runtime.shutdown().await;
    }
}

#[test]
fn daemon_terminal_delivery_does_not_refresh_existing_targeted_toast() {
    assert_terminal_delivery_keeps_toast(false);
}

#[test]
fn daemon_delayed_terminal_delivery_does_not_refresh_existing_targeted_toast() {
    assert_terminal_delivery_keeps_toast(true);
}

/// Keeps an existing target stable when delivery only targets the external client.
fn assert_terminal_delivery_keeps_toast(delayed: bool) {
    use maestro_daemon::app::state::{ToastKind, ToastNotification, ToastTarget};
    let mut app = context_app("/__maestro_original__".into(), delayed);
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let workspace_id = app.state.topology.workspaces[0].id.clone();
    app.state.terminals.get_mut(&terminal_id).unwrap().cwd = "/__maestro_projects__".into();
    app.state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Terminal;
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Codex),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: Instant::now(),
    });
    app.state.toast = Some(ToastNotification {
        kind: ToastKind::Finished,
        title: "codex finished".into(),
        context: "__maestro_original__ · 1".into(),
        position: None,
        target: Some(ToastTarget {
            workspace_id,
            pane_id: pane,
        }),
    });
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Codex),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: false,
        observed_at: Instant::now(),
    });
    if delayed {
        let deadline = app
            .state
            .next_pending_agent_notification_deadline()
            .unwrap();
        let deliveries = app.drain_due_agent_notifications(deadline);
        assert_eq!(deliveries.len(), 1);
        assert!(deliveries[0].client_notification.is_some());
    }
    assert_eq!(
        app.state.toast.as_ref().map(|toast| toast.context.as_str()),
        Some("__maestro_original__ · 1")
    );
}

/// Observes a running replacement shell and its actual injected public identities.
async fn assert_shell_identity(
    app: &TestApp,
    terminal: &maestro_workspace::terminal_id::TerminalId,
) {
    let runtime = app.terminal_runtimes.get(terminal).unwrap();
    runtime.send_bytes(b"printf '\\nspawned:<%s>|<%s>|<%s>\\n' \"$MAESTRO_WORKSPACE_ID\" \"$MAESTRO_TAB_ID\" \"$MAESTRO_PANE_ID\"\n".to_vec()).await.unwrap();
    loop {
        let _ = app.launch.render_dirty.take();
        if runtime
            .visible_text()
            .contains("spawned:<w1>|<w1:t1>|<w1:p1>")
        {
            break;
        }
        app.launch.render_notify.notified().await;
    }
    assert_eq!(runtime.cwd(), Some(temporary_root()));
    assert_eq!(runtime.current_size(), app.state.estimate_pane_size());
    assert!(
        !app.event_hub
            .events_after(0)
            .iter()
            .any(|(_, event)| matches!(
                event,
                maestro_protocol::events::EventData::PaneExited { .. }
            ))
    );
}

#[test]
fn failed_shell_respawn_keeps_normal_pane_exit() {
    let mut app = context_app(temporary_root(), false);
    app.config.terminal.default_shell = "/missing-maestro-shell-fixture/bin/sh".into();
    app.config.terminal.shell_mode = maestro_daemon::settings::model::ShellModeConfig::Login;
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let terminal_id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    app.state
        .terminals
        .get_mut(&terminal_id)
        .unwrap()
        .respawn_shell_on_exit = true;
    app.handle_internal_event(AppEvent::PaneDied { pane_id: pane });
    assert!(app.state.find_pane(pane).is_none());
    assert!(!app.state.terminals.contains_key(&terminal_id));
    assert!(app.terminal_runtimes.is_empty());
    assert!(
        app.event_hub
            .events_after(0)
            .iter()
            .any(|(_, event)| matches!(
                event,
                maestro_protocol::events::EventData::PaneExited { .. }
            ))
    );
}

#[test]
fn daemon_clipboard_write_event_shows_feedback_toast() {
    let mut app = context_app(temporary_root(), false);
    app.handle_internal_event(AppEvent::ClipboardWrite {
        content: b"copied".to_vec(),
    });
    assert!(app.state.toast.is_none());
    assert_eq!(
        app.state.copy_feedback.as_ref().unwrap().message,
        "copied to clipboard"
    );
    assert!(app.copy_feedback_deadline.is_some());
}

#[test]
fn daemon_clipboard_feedback_can_be_disabled() {
    let mut app = context_app(temporary_root(), false);
    app.state.toast_config.clipboard.enabled = false;
    app.handle_internal_event(AppEvent::ClipboardWrite {
        content: b"copied".to_vec(),
    });
    assert!(app.state.copy_feedback.is_none());
    assert!(app.copy_feedback_deadline.is_none());
}

#[test]
fn daemon_clipboard_feedback_does_not_replace_notification_toast() {
    let mut app = context_app(temporary_root(), false);
    app.state.toast = Some(maestro_daemon::app::state::ToastNotification {
        kind: maestro_daemon::app::state::ToastKind::NeedsAttention,
        title: "agent needs attention".into(),
        context: "background · 2".into(),
        position: None,
        target: None,
    });
    let original = app.state.toast.clone();
    app.handle_internal_event(AppEvent::ClipboardWrite {
        content: b"copied".to_vec(),
    });
    assert_eq!(app.state.toast, original);
    assert_eq!(
        app.state
            .copy_feedback
            .as_ref()
            .map(|feedback| feedback.message.as_str()),
        Some("copied to clipboard")
    );
}

#[test]
fn clipboard_feedback_expires_at_its_deadline_and_disabling_clears_it() {
    let mut app = context_app(temporary_root(), false);
    app.handle_internal_event(AppEvent::ClipboardWrite {
        content: b"copied".to_vec(),
    });
    let deadline = app.copy_feedback_deadline.unwrap();
    assert!(
        !app.expire_copy_feedback(
            deadline
                .checked_sub(std::time::Duration::from_nanos(1))
                .unwrap()
        )
    );
    assert!(app.expire_copy_feedback(deadline));
    assert!(app.state.copy_feedback.is_none());
    assert!(app.copy_feedback_deadline.is_none());
    assert!(!app.expire_copy_feedback(deadline));
    app.handle_internal_event(AppEvent::ClipboardWrite {
        content: Vec::new(),
    });
    app.state.toast_config.clipboard.enabled = false;
    app.handle_internal_event(AppEvent::ClipboardWrite {
        content: Vec::new(),
    });
    assert!(app.state.copy_feedback.is_none());
    assert!(app.copy_feedback_deadline.is_none());
}

#[test]
fn daemon_internal_event_drain_limits_work_per_tick() {
    let mut app = context_app(temporary_root(), false);
    let (sender, receiver) = mpsc::channel(65);
    app.launch.events = sender;
    app.event_rx = receiver;
    for index in 0..=64 {
        app.launch
            .events
            .try_send(AppEvent::UpdateReady {
                version: format!("2.0.{index}"),
                install_command: "maestro install".into(),
            })
            .unwrap();
    }
    assert!(app.drain_internal_events());
    assert_eq!(app.state.update_available.as_deref(), Some("2.0.63"));
    assert!(app.event_rx.try_recv().is_ok());
}

#[test]
fn native_workspace_creation_retains_one_runtime_and_rolls_back_spawn_failure() {
    if run_alone("native_workspace_creation_retains_one_runtime_and_rolls_back_spawn_failure")
        .unwrap()
    {
        return;
    }
    TestProcess::named("daemon_events::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(check_workspace_creation(args));
            Ok::<_, std::io::Error>(())
        })
        .unwrap();
}

/// Exercises the real consuming topology callback, not a replacement spawner.
async fn check_workspace_creation(args: &[String]) {
    let mut config = Config::default();
    config.terminal.default_shell = "/bin/sh".into();
    config.terminal.new_cwd = maestro_daemon::settings::model::NewTerminalCwdConfig::Path(
        temporary_root().display().to_string(),
    );
    let (launch, receiver) = launch_inputs(args, support::manifest_cache());
    let mut app = App::new(
        &config,
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        maestro_daemon::api::event_hub::EventHub::default(),
    );
    assert!(app.ensure_default_workspace());
    assert!(!app.ensure_default_workspace());
    support::assert_identity_invariants(&app.state);
    let workspace = &app.state.topology.workspaces[0];
    let root = workspace.tabs[0].root_pane;
    let terminal = workspace.terminal_id(root).unwrap();
    assert_eq!(app.state.terminals.len(), 1);
    assert_eq!(app.terminal_runtimes.len(), 1);
    assert!(app.terminal_runtimes.get(terminal).is_some());
    assert_eq!(app.state.mode, maestro_daemon::app::state::Mode::Terminal);
    assert!(app.state.session_dirty);
    app.config.terminal.default_shell = "/missing-maestro-shell-fixture/bin/sh".into();
    app.state.session_dirty = false;
    assert!(
        app.create_workspace_with_options(temporary_root(), false)
            .is_err()
    );
    assert_eq!(app.state.topology.workspaces.len(), 1);
    assert_eq!(app.state.terminals.len(), 1);
    assert_eq!(app.terminal_runtimes.len(), 1);
    assert!(!app.state.session_dirty);
    support::assert_identity_invariants(&app.state);
    for (_, runtime) in app.terminal_runtimes.drain_for_handoff() {
        runtime.shutdown().await;
    }
}

#[test]
fn agent_event_publication_keeps_complete_ordered_payloads() {
    use maestro_protocol::{events::EventData, schema::AgentStatus};
    let mut app = context_app("/payloads".into(), false);
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let event = || AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Codex),
        state: AgentState::Working,
        visible_blocker: false,
        visible_working: true,
        process_exited: false,
        observed_at: Instant::now(),
    };
    app.handle_internal_event(event());
    assert_initial_agent_events(&app);
    let sequence = app.event_hub.current_sequence();
    app.handle_internal_event(event());
    assert!(app.event_hub.events_after(sequence).is_empty());
    let labels = std::collections::HashMap::from([("blocked".into(), "approval".into())]);
    app.state
        .terminals
        .values_mut()
        .next()
        .unwrap()
        .set_agent_metadata(maestro_daemon::terminal::state::AgentMetadataReport {
            source: "test".into(),
            agent_label: Some("codex".into()),
            applies_to_source: None,
            title: Some("review".into()),
            display_agent: Some("planner".into()),
            state_labels: labels.clone(),
            clear_title: false,
            clear_display_agent: false,
            clear_state_labels: false,
            ttl: None,
            seq: None,
        })
        .unwrap();
    app.handle_internal_event(AppEvent::StateChanged {
        pane_id: pane,
        agent: Some(Agent::Codex),
        state: AgentState::Blocked,
        visible_blocker: true,
        visible_working: false,
        process_exited: false,
        observed_at: Instant::now(),
    });
    let events = app.event_hub.events_after(sequence);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].1,
        EventData::PaneAgentStatusChanged {
            pane_id: "w1:p1".into(),
            workspace_id: "w1".into(),
            agent_status: AgentStatus::Blocked,
            agent: Some("codex".into()),
            title: Some("review".into()),
            display_agent: Some("planner".into()),
            state_labels: labels,
        }
    );
}

/// Initial identity precedes its complete execution-state payload.
fn assert_initial_agent_events(app: &TestApp) {
    use maestro_protocol::{events::EventData, schema::AgentStatus};
    let events = app.event_hub.events_after(0);
    assert_eq!(events.len(), 2);
    assert_eq!(
        events[0].1,
        EventData::PaneAgentDetected {
            pane_id: "w1:p1".into(),
            workspace_id: "w1".into(),
            agent: Some("codex".into()),
            released: false,
            final_status: None,
        }
    );
    assert_eq!(
        events[1].1,
        EventData::PaneAgentStatusChanged {
            pane_id: "w1:p1".into(),
            workspace_id: "w1".into(),
            agent_status: AgentStatus::Working,
            agent: Some("codex".into()),
            title: None,
            display_agent: None,
            state_labels: std::collections::HashMap::new(),
        }
    );
}
