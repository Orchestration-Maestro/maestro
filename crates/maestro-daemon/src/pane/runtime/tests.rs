//! Real commands through the runtime's public process and terminal owners.
use super::*;
use crate::pane::launch_env::PaneLaunchEnv;
use maestro_test_directory::temporary_root;
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
        socket: temporary_root().join("maestro-test.sock"),
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
                    temporary_root(),
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
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let path = scratch.path().join("cwd");
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
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    let base = scratch.path();
    let path = base.join("cwd");
    std::fs::create_dir_all(&path).unwrap();
    let script = format!("cd '{}'; printf 'CWD-READY'; read -r line", path.display());
    with_runtime(name, &script, |runtime, _, notify| async move {
        wait_text(&runtime, &notify, "CWD-READY").await;
        assert_eq!(runtime.cwd(), Some(path.clone()));
        std::fs::set_permissions(base, std::fs::Permissions::from_mode(0o0)).unwrap();
        let observed = runtime.cwd();
        std::fs::set_permissions(base, std::fs::Permissions::from_mode(0o700)).unwrap();
        runtime.shutdown().await;
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
        let cwd = temporary_root();
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

/// Checks the nondefault negotiation before comparing its imported snapshot.
fn distinctive_handoff_modes(runtime: &PaneRuntime) -> crate::pane::terminal::InputState {
    let input = runtime.input_state().unwrap();
    assert!(bool::from(input.application_cursor));
    assert!(bool::from(input.bracketed_paste));
    assert!(bool::from(input.focus_reporting));
    assert!(bool::from(input.modify_other_keys));
    assert!(!bool::from(input.mouse_alternate_scroll));
    assert_eq!(
        input.mouse_protocol_mode,
        crate::input::MouseProtocolMode::ButtonMotion
    );
    assert_eq!(
        input.mouse_protocol_encoding,
        crate::input::MouseProtocolEncoding::Sgr
    );
    assert_eq!(
        runtime.keyboard_protocol(),
        crate::input::KeyboardProtocol::Kitty { flags: 5 }
    );
    input
}

#[test]
fn runtime_handoff_resumes_the_same_child_with_retained_modes_and_history() {
    let name = "runtime_handoff_resumes_the_same_child_with_retained_modes_and_history";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(
        name,
        "printf '\\033[>5u\\033[>4;2m\\033[?1h\\033[?2004h\\033[?1004h\\033[?1002h\\033[?1006h\\033[?1007lBEFORE-HANDOFF'; read -r line; printf 'AFTER-HANDOFF'; read -r line",
        |mut runtime, _, notify| async move {
            wait_text(&runtime, &notify, "BEFORE-HANDOFF").await;
            let input = distinctive_handoff_modes(&runtime);
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
            assert_eq!(imported.input_state(), Some(input));
            assert_eq!(
                imported.keyboard_protocol(),
                crate::input::KeyboardProtocol::Kitty { flags: 5 }
            );
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
fn runtime_cursor_projection_gates_clips_and_translates_the_viewport() {
    let name = "runtime_cursor_projection_gates_clips_and_translates_the_viewport";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(name, "read -r line", |runtime, _, _| async move {
        use maestro_workspace::layout::Rect;
        runtime.seed_history_ansi("\x1b[3;5H\x1b[5 q");
        let area = Rect::new(10, 20, 8, 6);
        assert!(runtime.cursor_state(area, false).is_none());
        assert!(
            runtime
                .cursor_state(Rect::new(10, 20, 4, 6), true)
                .is_none()
        );
        assert!(
            runtime
                .cursor_state(Rect::new(10, 20, 8, 2), true)
                .is_none()
        );
        let cursor = runtime.cursor_state(area, true).unwrap();
        assert_eq!(
            (cursor.x, cursor.y, cursor.visible, cursor.shape),
            (14, 22, true, 5)
        );
        runtime.seed_history_ansi("\x1b[?25l");
        assert!(!runtime.cursor_state(area, true).unwrap().visible);
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_pixel_size_requires_nonzero_checked_geometry() {
    let name = "runtime_pixel_size_requires_nonzero_checked_geometry";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(name, "read -r line", |runtime, _, _| async move {
        assert_eq!(runtime.pixel_size(), None);
        runtime.resize(12, 40, 10, 20);
        assert_eq!(runtime.pixel_size(), Some((400, 240)));
        for (width, height) in [(0, 20), (10, 0), (u32::MAX, 20), (10, u32::MAX)] {
            runtime.resize(12, 40, width, height);
            assert_eq!(runtime.pixel_size(), None);
        }
        runtime.shutdown().await;
    });
}

#[test]
fn runtime_alternate_scroll_encodes_only_vertical_keys_in_the_selected_mode() {
    let name = "runtime_alternate_scroll_encodes_only_vertical_keys_in_the_selected_mode";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(name, "read -r line", |runtime, _, _| async move {
        use maestro_protocol::frames::ClientMouseKind::{
            Moved, ScrollDown, ScrollLeft, ScrollRight, ScrollUp,
        };
        assert_eq!(runtime.encode_alternate_scroll(ScrollUp), None);
        runtime.seed_history_ansi("\x1b[?1049h\x1b[?1007h");
        assert_eq!(
            runtime.encode_alternate_scroll(ScrollUp),
            Some(b"\x1b[A".to_vec())
        );
        assert_eq!(
            runtime.encode_alternate_scroll(ScrollDown),
            Some(b"\x1b[B".to_vec())
        );
        for kind in [ScrollLeft, ScrollRight, Moved] {
            assert_eq!(runtime.encode_alternate_scroll(kind), None);
        }
        runtime.seed_history_ansi("\x1b[?1h");
        assert_eq!(
            runtime.encode_alternate_scroll(ScrollUp),
            Some(b"\x1bOA".to_vec())
        );
        assert_eq!(
            runtime.encode_alternate_scroll(ScrollDown),
            Some(b"\x1bOB".to_vec())
        );
        runtime.seed_history_ansi("\x1b[?1003h");
        assert_eq!(runtime.encode_alternate_scroll(ScrollUp), None);
        runtime.seed_history_ansi("\x1b[?1003l\x1b[?1007l");
        assert_eq!(runtime.encode_alternate_scroll(ScrollUp), None);
        runtime.shutdown().await;
    });
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

/// Observes shutdown completion before releasing any bounded event capacity.
async fn shutdown_with_full_event_queue(
    mut runtime: PaneRuntime,
    mut events: mpsc::Receiver<AppEvent>,
) {
    use std::{
        future::Future,
        task::{Context, Poll, Waker},
    };
    runtime.stop_detection().await;
    while events.try_recv().is_ok() {}
    for _ in 0..8 {
        runtime
            .shared
            .events
            .try_send(AppEvent::TerminalBell {
                pane_id: PaneId::from_raw(42),
                count: 1,
            })
            .unwrap();
    }
    assert_eq!(events.capacity(), 0);
    let shutdown = runtime.shutdown();
    tokio::pin!(shutdown);
    assert!(
        matches!(
            shutdown
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(())
        ),
        "shutdown must not wait for event-queue capacity"
    );
    let mut bells = 0;
    loop {
        match events.recv().await.unwrap() {
            AppEvent::TerminalBell { count: 1, .. } => bells += 1,
            AppEvent::PaneDied { .. } => break,
            event => panic!("unexpected event: {event:?}"),
        }
    }
    assert_eq!(bells, 8, "queued events must not be lost");
}

#[test]
fn spawned_shutdown_completes_with_a_full_unpolled_event_queue() {
    let name = "spawned_shutdown_completes_with_a_full_unpolled_event_queue";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(name, "read -r line", |runtime, events, _| async move {
        shutdown_with_full_event_queue(runtime, events).await;
    });
}

#[test]
fn imported_shutdown_completes_with_a_full_unpolled_event_queue() {
    let name = "imported_shutdown_completes_with_a_full_unpolled_event_queue";
    if run_alone(name).unwrap() {
        return;
    }
    with_runtime(name, "read -r line", |runtime, _events, _| async move {
        runtime
            .pause_handoff_reader(std::time::Duration::from_secs(1))
            .await
            .unwrap();
        let fd = runtime.duplicate_handoff_fd().await.unwrap();
        let state = runtime.handoff_runtime_state(42);
        let (config, events, _) = config(&[]);
        let mut imported = PaneRuntime::from_handoff_fd(
            config,
            crate::handoff_runtime::ImportedHandoffRuntime {
                master_fd: fd,
                state,
            },
        )
        .unwrap();
        runtime.preserve_for_handoff().await.unwrap();
        imported.assume_handoff_ownership();
        imported.set_handoff_reader_paused(false).await.unwrap();
        shutdown_with_full_event_queue(imported, events).await;
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
fn runtime_shutdown_kills_members_after_the_shell_is_reaped() {
    let name = "runtime_shutdown_kills_members_after_the_shell_is_reaped";
    if run_alone(name).unwrap() {
        return;
    }
    let script = "/bin/sh -c 'trap \"\" HUP TERM; printf \"MEMBER:%s:READY\\n\" \"$$\"; exec sleep 600' & read -r line; exit 0";
    with_runtime(name, script, |runtime, mut events, notify| async move {
        wait_text(&runtime, &notify, ":READY").await;
        let member: u32 = runtime
            .visible_text()
            .split_once("MEMBER:")
            .unwrap()
            .1
            .split_once(":READY")
            .unwrap()
            .0
            .parse()
            .unwrap();
        runtime.send_bytes(b"\n".to_vec()).await.unwrap();
        while !matches!(events.recv().await, Some(AppEvent::PaneDied { .. })) {}
        assert!(
            runtime
                .child_wait_completed
                .as_ref()
                .unwrap()
                .load(Ordering::Acquire)
        );
        assert!(maestro_process::child_process::process_exists(member));
        runtime.shutdown().await;
        let stat = std::fs::read_to_string(format!("/proc/{member}/stat")).ok();
        maestro_process::child_process::signal_processes(
            &[member],
            maestro_process::child_process::Signal::Kill,
        );
        if let Some(stat) = stat {
            assert_eq!(
                stat[stat.rfind(')').unwrap() + 2..]
                    .split_whitespace()
                    .next(),
                Some("Z")
            );
        }
    });
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

#[test]
fn manifest_publication_resets_only_matching_retained_runtime_detection() {
    let name = "manifest_publication_resets_only_matching_retained_runtime_detection";
    if run_alone(name).unwrap() {
        return;
    }
    TestProcess::named("pane::runtime::tests::setup_entry")
        .unwrap()
        .nocapture()
        .single_threaded()
        .invoke_with(|args| {
            crate::tests::block_on(check_manifest_resets(args));
            Ok::<_, io::Error>(())
        })
        .unwrap();
}

/// Checks the actual production reset permit after each complete dispatch.
async fn check_manifest_resets(args: &[String]) {
    use crate::terminal::state::TerminalState;
    use maestro_detection::agents::Agent;
    let (mut runtime_config, receiver, _) = config(args);
    runtime_config.agent_detection = AgentDetection::Disabled;
    let mut app = manifest_test_app(&runtime_config, receiver);
    assert!(Arc::ptr_eq(&app.state.manifests, &runtime_config.manifests));
    let (pane, ()) = app
        .state
        .topology
        .new_with_tab("/tmp".into(), None, |_, _| Ok(()))
        .unwrap();
    runtime_config.pane_id = pane;
    let id = app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone();
    let runtime = PaneRuntime::spawn_shell_command(
        runtime_config,
        "read -r line",
        temporary_root(),
        &PaneLaunchEnv::default(),
    )
    .unwrap();
    app.terminal_runtimes.insert(id.clone(), runtime);
    let mut terminal = TerminalState::new(id.clone(), "/tmp".into());
    terminal.detected_agent = Some(Agent::Codex);
    app.state.terminals.insert(id.clone(), terminal);
    let orphan_id = app.state.topology.alloc_terminal_id();
    let mut orphan = TerminalState::new(orphan_id.clone(), "/tmp".into());
    orphan.detected_agent = Some(Agent::Codex);
    app.state.terminals.insert(orphan_id, orphan);
    assert_manifest_reset_selection(&mut app, &id);
    assert_hook_runtime_controls(&mut app, &id);
    app.terminal_runtimes.remove(&id).unwrap().shutdown().await;
}

/// Uses the real application construction path with the supplied runtime adapters.
fn manifest_test_app(
    runtime_config: &PaneRuntimeConfig,
    receiver: mpsc::Receiver<AppEvent>,
) -> crate::app::App<fn() -> u128> {
    use crate::{
        api::event_hub::EventHub,
        app::{App, construction::PaneLaunchInputs},
        settings::model::Config,
    };
    let launch = PaneLaunchInputs {
        no_session: true,
        setup_entry: runtime_config.setup_entry.clone(),
        socket: runtime_config.socket.clone(),
        executable: None,
        manifests: Arc::clone(&runtime_config.manifests),
        events: runtime_config.events.clone(),
        render_notify: Arc::clone(&runtime_config.render_notify),
        render_dirty: Arc::clone(&runtime_config.render_dirty),
        theme: runtime_config.theme,
        appearance: None,
        process_detection_mode: runtime_config.process_detection_mode,
    };
    App::new(
        &Config::default(),
        (|| 0) as fn() -> u128,
        launch,
        receiver,
        EventHub::default(),
    )
}

/// Compares actual reset readiness after matching, nonmatching and empty publications.
fn assert_manifest_reset_selection(
    app: &mut crate::app::App<fn() -> u128>,
    id: &maestro_workspace::terminal_id::TerminalId,
) {
    use maestro_detection::{
        agents::{Agent, AgentState},
        manifest_update::{ManifestUpdateCommit, ManifestUpdateStatus, ManifestVersion},
    };
    for (hook, agents, expected) in [
        (None, vec![], false),
        (None, vec![Agent::Claude], false),
        (None, vec![Agent::Codex], true),
        (Some("claude"), vec![Agent::Codex], false),
        (Some("claude"), vec![Agent::Claude], true),
        (Some("custom"), vec![Agent::Codex], true),
    ] {
        app.state.terminals.get_mut(id).unwrap().hook_authority =
            hook.map(|label| crate::terminal::state::HookAuthority {
                source: "test".into(),
                agent_label: label.into(),
                state: AgentState::Working,
                message: None,
                reported_at: std::time::Instant::now(),
                session_ref: None,
            });
        app.handle_internal_event(AppEvent::AgentDetectionManifestsUpdated {
            updated: agents
                .into_iter()
                .map(|agent| ManifestUpdateCommit {
                    agent,
                    version: ManifestVersion::parse("2026.06.10.1").unwrap(),
                })
                .collect(),
            status: ManifestUpdateStatus::default(),
        });
        assert_eq!(
            detection_reset_ready(app.terminal_runtimes.get(id).unwrap()),
            expected
        );
    }
}

/// Checks lifecycle pause and release routing on the actual retained runtime control.
fn assert_hook_runtime_controls(
    app: &mut crate::app::App<fn() -> u128>,
    id: &maestro_workspace::terminal_id::TerminalId,
) {
    use maestro_detection::agents::{Agent, AgentState};
    let pane = app.state.topology.workspaces[0].tabs[0].root_pane;
    app.state.terminals.insert(
        id.clone(),
        crate::terminal::state::TerminalState::new(id.clone(), "/tmp".into()),
    );
    let terminal = app.state.terminals.get_mut(id).unwrap();
    terminal.detected_agent = Some(Agent::Pi);
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:pi".into(),
        agent: "pi".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("live-session").unwrap(),
    });
    let updates = app.handle_internal_event(AppEvent::HookStateReported {
        pane_id: pane,
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        seq: Some(1),
        session_ref: Some(crate::agent_resume::AgentSessionRef::id("live-session").unwrap()),
    });
    assert!(!updates.is_empty());
    assert!(detection_reset_ready(
        app.terminal_runtimes.get(id).unwrap()
    ));
    app.state.terminals.get_mut(id).unwrap().hook_authority = None;
    app.state.terminals.get_mut(id).unwrap().detected_agent = None;
    app.handle_internal_event(AppEvent::HookStateReported {
        pane_id: pane,
        source: "custom".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        seq: Some(1),
        session_ref: None,
    });
    app.handle_internal_event(AppEvent::HookAgentReleased {
        pane_id: pane,
        source: "custom".into(),
        agent_label: "pi".into(),
        known_agent: Some(Agent::Pi),
        seq: Some(2),
    });
    assert!(detection_reset_ready(
        app.terminal_runtimes.get(id).unwrap()
    ));
}

/// Observes one retained detector reset permit without waiting or changing its owner.
fn detection_reset_ready(runtime: &PaneRuntime) -> bool {
    use std::{
        future::Future,
        task::{Context, Waker},
    };
    let reset = runtime.control.wait_for_reset();
    tokio::pin!(reset);
    reset
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_ready()
}
