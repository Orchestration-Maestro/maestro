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
fn assert_delayed_context(app: &mut App<fn() -> u128>) {
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
fn context_app(directory: std::path::PathBuf, delayed: bool) -> App<fn() -> u128> {
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
    }
}

/// Supplies the same existing pane runtime boundary used by native launches.
fn runtime_config(
    args: &[String],
    pane: maestro_workspace::layout::PaneId,
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
