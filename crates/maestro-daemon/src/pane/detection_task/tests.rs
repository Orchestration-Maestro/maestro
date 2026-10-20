//! Detector lifecycle and publication observations.

use super::*;
use std::{
    future::Future,
    task::{Context, Poll, Waker},
};

#[test]
fn process_observation_precedes_grace_and_fresh_screen_publication() {
    crate::tests::block_on(async {
        let (config, mut events) = config();
        let now = Instant::now();
        config
            .terminal
            .process_pty_bytes(config.pane_id, 0, b"\x1b]0;Action Required\x07", || None);
        let mut detector = Detector::new(now);
        let input = detector.probe_input(None, Some(42), now);
        assert!(
            detector
                .apply_probe(
                    &config,
                    ProcessProbeResult {
                        process_group_id: Some(42),
                        foreground_is_pane_shell: false,
                        agent: Some(Agent::Codex),
                        process_name: Some("codex".into()),
                    },
                    input,
                    now
                )
                .await
        );
        assert!(
            matches!(events.recv().await, Some(AppEvent::AgentProcessDetected {
        agent: Agent::Codex, pane_id, observed_at,
    }) if pane_id == config.pane_id && observed_at == now)
        );
        assert_eq!(config.terminal.agent_osc_title(), "");
        assert!(!detector.screen_allowed(false, now));
        assert!(!detector.screen_allowed(false, now + AGENT_STARTUP_GRACE_WINDOW));
        let after = now + AGENT_STARTUP_GRACE_WINDOW + Duration::from_millis(300);
        assert!(detector.screen_allowed(false, after));
        config
            .terminal
            .process_pty_bytes(config.pane_id, 0, b"\x1b]0;Action Required\x07", || None);
        let input = detector.probe_input(None, Some(42), after);
        detector.scan_screen(&config, input, false, after).await;
        assert!(matches!(events.recv().await, Some(AppEvent::StateChanged {
        agent: Some(Agent::Codex), state: AgentState::Blocked,
        visible_blocker: true, process_exited: false, observed_at, ..
    }) if observed_at == after));
    });
}

/// Independent pane and supplied compiled manifests.
fn config() -> (DetectionTaskConfig, tokio::sync::mpsc::Receiver<AppEvent>) {
    let (events, receiver) = tokio::sync::mpsc::channel(8);
    (
        DetectionTaskConfig {
            pane_id: PaneId::from_raw(42),
            child_pid: Arc::new(AtomicU32::new(0)),
            terminal: Arc::new(PaneTerminal::new(
                maestro_terminal::Terminal::new(80, 24, 100).unwrap(),
            )),
            detection_content_seq: Arc::new(AtomicU64::new(0)),
            control: Arc::new(DetectionControl::default()),
            state_events: events,
            manifests: Arc::new(RwLock::new(ManifestCache::new(None, None).unwrap())),
            process_detection_mode: ProcessDetectionMode::Native,
        },
        receiver,
    )
}

#[test]
fn runtime_set_full_lifecycle_authority_active_notifies_only_on_activation_transitions() {
    crate::tests::block_on(async {
        let control = DetectionControl::default();
        control.set_full_lifecycle_authority_active(true);
        control.reset.notified().await;
        control.set_full_lifecycle_authority_active(true);
        let notification = control.reset.notified();
        tokio::pin!(notification);
        assert!(matches!(
            notification
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        control.set_full_lifecycle_authority_active(false);
        assert!(matches!(
            notification
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        control.set_full_lifecycle_authority_active(true);
        notification.await;
    });
}

#[test]
fn process_completion_bypasses_authority_and_grace_before_replacement() {
    crate::tests::block_on(async {
        let (config, mut events) = config();
        let now = Instant::now();
        let mut detector = Detector::new(now);
        let identified = ProcessProbeResult {
            process_group_id: Some(42),
            foreground_is_pane_shell: false,
            agent: Some(Agent::Claude),
            process_name: Some("claude".into()),
        };
        let input = detector.probe_input(None, Some(42), now);
        detector
            .apply_probe(&config, identified.clone(), input, now)
            .await;
        assert!(matches!(
            events.recv().await,
            Some(AppEvent::AgentProcessDetected {
                agent: Agent::Claude,
                ..
            })
        ));
        config.control.set_full_lifecycle_authority_active(true);
        let input = detector.probe_input(None, Some(42), now);
        let shell = ProcessProbeResult {
            process_group_id: Some(42),
            foreground_is_pane_shell: true,
            agent: None,
            process_name: None,
        };
        assert!(!detector.apply_probe(&config, shell, input, now).await);
        config.terminal.process_pty_bytes(
            config.pane_id,
            0,
            b"showing detailed transcript\r\nctrl+o to toggle",
            || None,
        );
        assert!(detector.screen_allowed(true, now));
        detector.scan_screen(&config, input, false, now).await;
        assert!(matches!(
            events.recv().await,
            Some(AppEvent::StateChanged {
                agent: Some(Agent::Claude),
                state: AgentState::Idle,
                process_exited: true,
                ..
            })
        ));
        let input = detector.probe_input(None, Some(42), now);
        assert!(detector.apply_probe(&config, identified, input, now).await);
        assert!(matches!(
            events.recv().await,
            Some(AppEvent::AgentProcessDetected {
                agent: Agent::Claude,
                ..
            })
        ));
    });
}

#[test]
fn task_abort_releases_its_pane_and_control_owners() {
    crate::tests::block_on(async {
        let (config, mut events) = config();
        let terminal = Arc::downgrade(&config.terminal);
        let control = Arc::downgrade(&config.control);
        let handle = spawn_basic_detection_task(config);
        assert!(terminal.upgrade().is_some());
        handle.abort();
        assert!(handle.await.unwrap_err().is_cancelled());
        assert!(terminal.upgrade().is_none());
        assert!(control.upgrade().is_none());
        assert!(events.recv().await.is_none());
    });
}

#[test]
fn screen_content_change_drives_the_tasks_same_group_acquisition_schedule() {
    crate::tests::block_on(async {
        let (config, _events) = config();
        let now = Instant::now();
        let mut detector = Detector::new(now);
        detector.has_process_probe = true;
        let input = detector.probe_input(None, None, now);
        assert!(!should_probe_foreground_job(input));
        config
            .terminal
            .process_pty_bytes(config.pane_id, 0, b"starting wrapper", || None);
        detector.tick(&config, now, None).await;
        let due = now + super::super::probe::schedule::PROCESS_ACQUISITION_FAST_RECHECK;
        assert!(should_probe_foreground_job(
            detector.probe_input(None, None, due)
        ));
    });
}

#[test]
fn graceful_release_notifies_and_expires_without_extending_on_observation() {
    crate::tests::block_on(async {
        let control = DetectionControl::default();
        let before = tokio::time::Instant::now().into_std();
        control.begin_graceful_release(Agent::Codex);
        let after = tokio::time::Instant::now().into_std();
        control.reset.notified().await;
        let until = control.pending_release.lock().unwrap().unwrap().until;
        assert!(until.duration_since(before) >= Duration::from_secs(1));
        assert!(until.duration_since(after) <= Duration::from_secs(1));
        let previous = until.checked_sub(Duration::from_nanos(1)).unwrap();
        assert_eq!(control.active_pending_release(previous), Some(Agent::Codex));
        assert_eq!(control.active_pending_release(previous), Some(Agent::Codex));
        assert_eq!(control.active_pending_release(until), None);
        assert_eq!(control.active_pending_release(until), None);
    });
}
