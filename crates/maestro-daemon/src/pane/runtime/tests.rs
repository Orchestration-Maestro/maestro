//! Real commands through the runtime's public process and terminal owners.
use super::*;
use crate::pane::launch_env::PaneLaunchEnv;
use maestro_test_process::{TestProcess, run_alone};
use std::{ffi::OsString, io};

#[test]
fn setup_entry() {
    if maestro_process::pty::setup_entry().is_some() {
        std::process::exit(1);
    }
}

/// Supplied runtime configuration shared by native runtime witnesses.
fn config(args: &[String]) -> (PaneRuntimeConfig, mpsc::Receiver<AppEvent>, Arc<Notify>) {
    let (events, receiver) = mpsc::channel(8);
    let notify = Arc::new(Notify::new());
    let config = PaneRuntimeConfig {
        pane_id: PaneId::from_raw(42),
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
        render_notify: Arc::clone(&notify),
        render_dirty: Arc::new(RenderSignal::new()),
        manifests: Arc::new(std::sync::RwLock::new(
            ManifestCache::new(None, None).unwrap(),
        )),
        process_detection_mode: ProcessDetectionMode::Native,
        agent_detection: AgentDetection::Enabled,
        setup_entry: maestro_process::pty::PtySetupEntry {
            executable: std::env::current_exe().unwrap(),
            args: args.iter().map(OsString::from).collect(),
        },
        socket: std::env::temp_dir().join("maestro-test.sock"),
        executable: None,
    };
    (config, receiver, notify)
}

/// Drives one real terminal transaction in an isolated checked test process.
fn with_runtime<F, Fut>(name: &str, command: &str, observe: F)
where
    F: FnOnce(PaneRuntime, mpsc::Receiver<AppEvent>, Arc<Notify>) -> Fut,
    Fut: std::future::Future<Output = ()>,
{
    TestProcess::named("pane::runtime::tests::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            crate::tests::block_on(async {
                let (config, receiver, notify) = config(args);
                let runtime = PaneRuntime::spawn_shell_command(
                    config,
                    command,
                    std::env::temp_dir(),
                    &PaneLaunchEnv::default(),
                )
                .unwrap();
                wait_text(&runtime, &notify, "\nrunning 1 test\n").await;
                assert!(runtime.visible_text().starts_with("\nrunning 1 test\n"));
                observe(runtime, receiver, notify).await;
            });
            Ok::<_, io::Error>(())
        })
        .unwrap_or_else(|error| panic!("{name}: {error}"));
}

#[test]
fn runtime_spawned_pty_reader_aggregates_terminal_bells() {
    let name = "runtime_spawned_pty_reader_aggregates_terminal_bells";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(
        name,
        "printf '\\a\\a'; read -r line",
        |runtime, mut events, _| async move {
            loop {
                if let Some(AppEvent::TerminalBell { pane_id, count }) = events.recv().await {
                    assert_eq!(pane_id, PaneId::from_raw(42));
                    assert_eq!(count, 2);
                    break;
                }
            }
            runtime.shutdown().await;
        },
    );
}

/// Waits for a child-produced marker through the public terminal read.
async fn wait_text(runtime: &PaneRuntime, notify: &Notify, marker: &str) {
    loop {
        let _ = runtime.shared.render_dirty.take();
        if runtime.visible_text().contains(marker) {
            return;
        }
        notify.notified().await;
    }
}

#[test]
fn runtime_handoff_history_ansi_captures_primary_screen() {
    let name = "runtime_handoff_history_ansi_captures_primary_screen";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(
        name,
        "printf 'handoff-primary-history\\r\\n'; read -r line",
        |runtime, _, notify| async move {
            wait_text(&runtime, &notify, "handoff-primary-history").await;
            assert!(
                runtime
                    .handoff_history_ansi()
                    .unwrap()
                    .contains("handoff-primary-history")
            );
            runtime.shutdown().await;
        },
    );
}

#[test]
fn runtime_handoff_history_ansi_skips_alternate_screen() {
    let name = "runtime_handoff_history_ansi_skips_alternate_screen";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(
        name,
        "read -r line; printf '\\033[?1049halt-screen'; read -r line",
        |runtime, _, notify| async move {
            runtime.io.write_user_input(b"\n".to_vec()).await.unwrap();
            wait_text(&runtime, &notify, "alt-screen").await;
            assert!(runtime.handoff_history_ansi().is_none());
            runtime.shutdown().await;
        },
    );
}

#[test]
fn runtime_truncate_handoff_history_keeps_recent_utf8_boundary() {
    assert_eq!(
        super::handoff::truncate_handoff_history(format!("old\n{}\nrecent\n", "é".repeat(8)), 20),
        "recent\n"
    );
}

#[test]
fn runtime_truncate_handoff_history_drops_partial_long_line() {
    assert!(
        super::handoff::truncate_handoff_history(format!("old\n{}", "x".repeat(64)), 12).is_empty()
    );
}

#[test]
fn runtime_cwd_returns_accepted_report_without_rechecking_filesystem() {
    let name = "runtime_cwd_returns_accepted_report_without_rechecking_filesystem";
    if run_alone(name).unwrap() {
        return;
    }
    let path = std::env::temp_dir().join(format!("maestro-cwd-{}", std::process::id()));
    std::fs::create_dir(&path).unwrap();
    let script = format!(
        "printf '\\033]7;file://{}\\033\\\\'; read -r line",
        path.display()
    );
    with_runtime(name, &script, |runtime, mut events, _| async move {
        loop {
            if let Some(AppEvent::TerminalCwdReported { cwd, .. }) = events.recv().await {
                assert_eq!(cwd, path);
                break;
            }
        }
        std::fs::remove_dir(&path).unwrap();
        assert_eq!(runtime.cwd(), Some(path));
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_shutdown_liveness_treats_reaped_direct_child_as_gone() {
    assert!(!super::lifecycle::process_alive_for_shutdown(
        42,
        42,
        true,
        |_| true
    ));
}
#[test]
fn runtime_shutdown_liveness_keeps_unreaped_direct_child_alive() {
    assert!(super::lifecycle::process_alive_for_shutdown(
        42,
        42,
        false,
        |_| true
    ));
}
#[test]
fn runtime_shutdown_liveness_keeps_other_session_processes_alive() {
    assert!(super::lifecycle::process_alive_for_shutdown(
        43,
        42,
        true,
        |_| true
    ));
}
#[test]
fn runtime_shutdown_liveness_treats_missing_process_as_gone() {
    assert!(!super::lifecycle::process_alive_for_shutdown(
        43,
        42,
        false,
        |_| false
    ));
}

#[test]
fn runtime_process_cwd_does_not_require_traversing_the_directory_path() {
    use std::os::unix::fs::PermissionsExt;
    let name = "runtime_process_cwd_does_not_require_traversing_the_directory_path";
    if run_alone(name).unwrap() {
        return;
    }
    let base = std::env::temp_dir().join(format!("maestro-process-cwd-{}", std::process::id()));
    let path = base.join("cwd");
    std::fs::create_dir_all(&path).unwrap();
    let script = format!("cd '{}'; printf 'CWD-READY'; read -r line", path.display());
    with_runtime(name, &script, |runtime, _, notify| async move {
        wait_text(&runtime, &notify, "CWD-READY").await;
        assert_eq!(runtime.cwd(), Some(path.clone()));
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o0)).unwrap();
        let observed = runtime.cwd();
        std::fs::set_permissions(&base, std::fs::Permissions::from_mode(0o700)).unwrap();
        runtime.shutdown().await;
        std::fs::remove_dir_all(&base).unwrap();
        assert_eq!(observed, Some(path));
    });
}

#[test]
fn runtime_follow_cwd_falls_back_to_reported_pane_cwd_without_foreground_group() {
    let name = "runtime_follow_cwd_falls_back_to_reported_pane_cwd_without_foreground_group";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(name, "read -r line", |runtime, _, _| async move {
        let cwd = std::env::temp_dir();
        runtime.shared.publish_cwd(cwd.clone());
        runtime.io.release_after_commit().await.unwrap();
        assert_eq!(runtime.follow_cwd().await, Some(cwd));
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_focus_events_are_forwarded_when_enabled() {
    let name = "runtime_focus_events_are_forwarded_when_enabled";
    if run_alone(name).unwrap() {
        return;
    }
    let script = "stty raw -echo; printf '\\033[?1004hFOCUS-READY'; bytes=$(dd bs=1 count=3 2>/dev/null | od -An -tx1); printf 'GOT:%s' \"$bytes\"; read -r line";
    with_runtime(name, script, |runtime, _, notify| async move {
        wait_text(&runtime, &notify, "FOCUS-READY").await;
        assert!(runtime.try_send_focus_event(FocusEvent::Gained));
        wait_text(&runtime, &notify, "GOT: 1b 5b 49").await;
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_focus_events_are_suppressed_when_disabled() {
    let name = "runtime_focus_events_are_suppressed_when_disabled";
    if run_alone(name).unwrap() {
        return;
    }
    let script = "stty raw -echo; printf 'FOCUS-OFF'; bytes=$(dd bs=1 count=3 2>/dev/null | od -An -tx1); printf 'GOT:%s' \"$bytes\"; read -r line";
    with_runtime(name, script, |runtime, _, notify| async move {
        wait_text(&runtime, &notify, "FOCUS-OFF").await;
        assert!(!runtime.try_send_focus_event(FocusEvent::Gained));
        runtime.send_bytes(b"xyz".to_vec()).await.unwrap();
        wait_text(&runtime, &notify, "GOT: 78 79 7a").await;
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_subscribed_idle_child_receives_color_scheme_transition() {
    let name = "runtime_subscribed_idle_child_receives_color_scheme_transition";
    if run_alone(name).unwrap() {
        return;
    }
    let script = "read -r line; stty raw -echo; printf '\\033[?2031hCOLOR-READY'; bytes=$(dd bs=1 count=9 2>/dev/null | od -An -tx1); printf 'GOT:%s' \"$bytes\"; read -r line";
    with_runtime(name, script, |runtime, _, notify| async move {
        runtime.apply_host_terminal_appearance(Some(HostAppearance::Dark));
        runtime.send_bytes(b"\n".to_vec()).await.unwrap();
        wait_text(&runtime, &notify, "COLOR-READY").await;
        runtime.apply_host_terminal_appearance(Some(HostAppearance::Light));
        wait_text(&runtime, &notify, "GOT: 1b 5b 3f 39 39 37 3b 32 6e").await;
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_handoff_runtime_state_captures_terminal_input_and_title_state() {
    let name = "runtime_handoff_runtime_state_captures_terminal_input_and_title_state";
    if run_alone(name).unwrap() {
        return;
    }
    let script = "printf '\\033[>5u\\033[>4;2m\\033[?1h\\033[?2004h\\033[?1004h\\033[?1002h\\033[?1006h\\033[?2031h\\033]2;✳ 修复🙂标题\\033\\\\STATE-READY'; read -r line";
    with_runtime(name, script, |runtime, _, notify| async move {
        wait_text(&runtime, &notify, "STATE-READY").await;
        runtime.clear_agent_osc_state();
        assert_eq!(runtime.agent_osc_title(), "");
        let state = runtime.handoff_runtime_state(12);
        assert_eq!(state.pane_id, 12);
        assert_eq!(state.keyboard_protocol_flags, 5);
        assert_eq!(state.terminal_title.as_deref(), Some("✳ 修复🙂标题"));
        let input = state.input_state.unwrap();
        assert!(!bool::from(input.alternate_screen));
        assert!(bool::from(input.application_cursor));
        assert!(bool::from(input.bracketed_paste));
        assert!(bool::from(input.focus_reporting));
        assert_eq!(
            input.mouse_protocol_mode,
            crate::input::MouseProtocolMode::ButtonMotion
        );
        assert_eq!(
            input.mouse_protocol_encoding,
            crate::input::MouseProtocolEncoding::Sgr
        );
        assert!(bool::from(input.mouse_alternate_scroll));
        assert!(bool::from(input.modify_other_keys));
        assert!(bool::from(input.color_scheme_reporting));
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_spawned_detection_observes_real_child_hint() {
    let name = "runtime_spawned_detection_observes_real_child_hint";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(
        name,
        "exec env MAESTRO_AGENT=codex /bin/sh -c 'printf DETECTOR-READY; read -r line'",
        |runtime, mut events, notify| async move {
            assert!(
                runtime.detect_handle.is_some(),
                "spawned panes must own their detector"
            );
            wait_text(&runtime, &notify, "DETECTOR-READY").await;
            loop {
                if let Some(AppEvent::AgentProcessDetected { pane_id, agent, .. }) =
                    events.recv().await
                {
                    assert_eq!(pane_id, PaneId::from_raw(42));
                    assert_eq!(agent, maestro_detection::agents::Agent::Codex);
                    break;
                }
            }
            runtime.shutdown().await;
        },
    );
}

#[test]
fn runtime_handoff_resumes_the_same_child_with_retained_modes_and_history() {
    let name = "runtime_handoff_resumes_the_same_child_with_retained_modes_and_history";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(
        name,
        "printf 'BEFORE-HANDOFF'; read -r line; printf 'AFTER-HANDOFF'; read -r line",
        |mut runtime, _, notify| async move {
            wait_text(&runtime, &notify, "BEFORE-HANDOFF").await;
            runtime
                .pause_handoff_reader(std::time::Duration::from_secs(1))
                .await
                .unwrap();
            let fd = runtime.duplicate_handoff_fd().await.unwrap();
            let mut state = runtime.handoff_runtime_state(77);
            state.initial_history_ansi = runtime.handoff_history_ansi();
            let pid = runtime.child_pid();
            let watcher = runtime.watcher.take().unwrap();
            let (config, mut events, notify) = config(&[]);
            let mut imported = PaneRuntime::from_handoff_fd(
                config,
                crate::handoff_runtime::ImportedHandoffRuntime {
                    master_fd: fd,
                    state,
                },
            )
            .unwrap();
            runtime.preserve_for_handoff().await.unwrap();
            assert_eq!(imported.child_pid(), pid);
            assert!(imported.visible_text().contains("BEFORE-HANDOFF"));
            imported.assume_handoff_ownership();
            imported.set_handoff_reader_paused(false).await.unwrap();
            imported.send_bytes(b"\n".to_vec()).await.unwrap();
            wait_text(&imported, &notify, "AFTER-HANDOFF").await;
            imported.send_bytes(b"\n".to_vec()).await.unwrap();
            loop {
                if let Some(AppEvent::PaneDied { pane_id }) = events.recv().await {
                    assert_eq!(pane_id, PaneId::from_raw(77));
                    break;
                }
            }
            imported.shutdown().await;
            watcher.await.unwrap();
        },
    );
}

#[test]
fn registry_retains_and_drains_live_runtime() {
    let name = "registry_retains_and_drains_live_runtime";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(name, "read -r line", |runtime, _, _| async move {
        let id = serde_json::from_str("\"term_runtime\"").unwrap();
        let mut registry = crate::terminal::runtime_registry::TerminalRuntimeRegistry::new();
        assert!(registry.insert(id, runtime).is_none());
        assert_eq!(registry.len(), 1);
        registry.set_handoff_readers_paused(true).await;
        registry.assume_handoff_ownership();
        assert_eq!(registry.iter().count(), 1);
        assert_eq!(registry.values().count(), 1);
        let (id, runtime) = registry.drain_for_handoff().next().unwrap();
        assert!(registry.is_empty());
        assert!(registry.get(&id).is_none());
        assert!(registry.remove(&id).is_none());
        runtime.set_handoff_reader_paused(false).await.unwrap();
        runtime.shutdown().await;
    });
}

#[test]
fn pane_death_publication_waits_for_capacity() {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    crate::tests::block_on(async {
        let pane_id = PaneId::from_raw(42);
        let (sender, mut receiver) = mpsc::channel(1);
        sender
            .try_send(AppEvent::TerminalBell { pane_id, count: 1 })
            .unwrap();
        let publication = AppEvent::PaneDied { pane_id }.publish(&sender);
        tokio::pin!(publication);
        assert!(matches!(
            publication
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        assert!(matches!(
            receiver.recv().await,
            Some(AppEvent::TerminalBell { .. })
        ));
        publication.await;
        assert!(
            matches!(receiver.recv().await, Some(AppEvent::PaneDied { pane_id: observed }) if observed == pane_id)
        );
    });
}

#[test]
fn runtime_child_death_reaches_the_workspace_queue() {
    let name = "runtime_child_death_reaches_the_workspace_queue";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(
        name,
        "read -r line; exit 0",
        |mut runtime, mut events, _| async move {
            runtime.send_bytes(b"\n".to_vec()).await.unwrap();
            loop {
                if let Some(AppEvent::PaneDied { pane_id }) = events.recv().await {
                    assert_eq!(pane_id, PaneId::from_raw(42));
                    break;
                }
            }
            runtime.watcher.take().unwrap().await.unwrap();
            runtime.shutdown().await;
        },
    );
}

#[test]
fn runtime_shutdown_kills_resistant_session_members() {
    let name = "runtime_shutdown_kills_resistant_session_members";
    if run_alone(name).unwrap() {
        return;
    }
    let script = "trap '' HUP TERM; /bin/sh -c 'trap \"\" HUP TERM; printf \"MEMBER:%s:READY\\n\" \"$$\"; while :; do read -r line; done' & wait";
    with_runtime(name, script, |runtime, _, notify| async move {
        wait_text(&runtime, &notify, ":READY").await;
        let pid = runtime.child_pid().unwrap();
        let text = runtime.visible_text();
        let member: u32 = text
            .split_once("MEMBER:")
            .unwrap()
            .1
            .split_once(":READY")
            .unwrap()
            .0
            .parse()
            .unwrap();
        assert_ne!(pid, member);
        assert!(maestro_process::child_process::process_exists(pid));
        assert!(maestro_process::child_process::process_exists(member));
        runtime.shutdown().await;
        assert!(!maestro_process::child_process::process_exists(pid));
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{member}/stat")) {
            assert_eq!(
                stat[stat.rfind(')').unwrap() + 2..]
                    .split_whitespace()
                    .next(),
                Some("Z")
            );
        }
    });
}
