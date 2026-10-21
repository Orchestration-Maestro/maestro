//! Internal event delivery uses live retained terminal runtimes.
#![cfg(test)]
#![cfg(unix)]

mod support;

use maestro_daemon::{
    app::{App, state::DaemonState},
    events::AppEvent,
    pane::{
        launch_env::PaneLaunchEnv,
        runtime::{AgentDetection, PaneRuntime, PaneRuntimeConfig},
    },
    render_signal::RenderSignal,
    settings::model::Config,
    terminal::runtime_registry::TerminalRuntimeRegistry,
    terminal_theme::TerminalTheme,
};
use maestro_detection::agents::{Agent, AgentState};
use maestro_process::pty::{PtySetupEntry, PtySize, foreground::ProcessDetectionMode};
use maestro_test_process::{TestProcess, run_alone};
use maestro_workspace::layout::PaneId;

/// Daemon owner driven by a deterministic public-identity clock.
type TestApp = App<fn() -> u128>;
use std::{ffi::OsString, sync::Arc, time::Instant};
use tokio::sync::{Notify, mpsc};

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
    TestProcess::named("setup_entry")
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
    let root = context_directories();
    let live = root.join("__maestro_projects__");
    let mut app = context_app(root.join("__maestro_original__"), delayed);
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let (config, mut events) = runtime_config(args, pane, Arc::clone(&app.state.manifests));
    let command = format!(
        "printf '\\033]7;file://{}\\033\\\\'; read -r line",
        live.display()
    );
    let runtime =
        PaneRuntime::spawn_shell_command(config, &command, live.clone(), &PaneLaunchEnv::default())
            .unwrap();
    loop {
        if let Some(AppEvent::TerminalCwdReported { cwd, .. }) = events.recv().await {
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
fn context_directories() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("maestro-toast-context-{}", std::process::id()));
    for name in ["__maestro_original__", "__maestro_projects__"] {
        let path = root.join(name);
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
    let mut state = DaemonState::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        support::manifest_cache(),
    );
    state
        .topology
        .new_with_tab(directory, None, |_, _| Ok(()))
        .unwrap();
    support::populate_terminals(&mut state);
    state.active = Some(0);
    support::assert_identity_invariants(&state);
    state.active = None;
    state.toast_config.delivery = maestro_daemon::settings::model::ToastDelivery::Workspace;
    state.toast_config.delay_seconds = u64::from(delayed);
    state.sound.enabled = false;
    App {
        state,
        terminal_runtimes: TerminalRuntimeRegistry::new(),
        overlay_panes: std::collections::HashMap::new(),
        event_hub: maestro_daemon::api::event_hub::EventHub::default(),
    }
}

/// Supplies the same existing pane runtime boundary used by native launches.
fn runtime_config(
    args: &[String],
    pane: PaneId,
    manifests: Arc<std::sync::RwLock<maestro_detection::manifest::ManifestCache>>,
) -> (PaneRuntimeConfig, mpsc::Receiver<AppEvent>) {
    let (events, receiver) = mpsc::channel(8);
    (
        PaneRuntimeConfig {
            pane_id: pane,
            size: PtySize {
                rows: 24,
                cols: 80,
                cell_width_px: 0,
                cell_height_px: 0,
            },
            scrollback_limit_bytes: 4096,
            theme: TerminalTheme::default(),
            appearance: None,
            events,
            render_notify: Arc::new(Notify::new()),
            render_dirty: Arc::new(RenderSignal::new()),
            manifests,
            process_detection_mode: ProcessDetectionMode::Native,
            agent_detection: AgentDetection::Disabled,
            setup_entry: PtySetupEntry {
                executable: std::env::current_exe().unwrap(),
                args: args.iter().map(OsString::from).collect(),
            },
            socket: std::env::temp_dir().join("maestro-context-test.sock"),
            executable: None,
        },
        receiver,
    )
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
    let temporary_file =
        std::env::temp_dir().join(format!("maestro-overlay-exit-{}", std::process::id()));
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
