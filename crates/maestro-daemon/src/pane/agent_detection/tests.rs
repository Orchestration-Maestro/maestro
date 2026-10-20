//! Detection screen and publication behavior.

use super::*;

#[test]
fn runtime_state_changed_event_waits_for_queue_space_instead_of_dropping() {
    crate::tests::block_on(async {
        use crate::events::AppEvent;
        use maestro_workspace::layout::PaneId;
        use std::{
            future::Future,
            task::{Context, Poll, Waker},
        };

        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let pane_id = PaneId::from_raw(42);
        let observed_at = Instant::now();
        tx.send(AppEvent::AgentProcessDetected {
            pane_id,
            agent: Agent::Codex,
            observed_at,
        })
        .await
        .unwrap();
        let expected = AppEvent::StateChanged {
            pane_id,
            agent: Some(Agent::Codex),
            state: AgentState::Idle,
            visible_blocker: false,
            visible_working: false,
            process_exited: true,
            observed_at,
        };
        let publish = expected.publish(&tx);
        tokio::pin!(publish);
        assert!(matches!(
            publish
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Pending
        ));
        assert!(matches!(
            rx.recv().await,
            Some(AppEvent::AgentProcessDetected { .. })
        ));
        publish.await;
        assert!(matches!(rx.recv().await, Some(AppEvent::StateChanged {
        pane_id: actual,
        agent: Some(Agent::Codex),
        state: AgentState::Idle,
        visible_blocker: false,
        visible_working: false,
        process_exited: true,
        observed_at: actual_time,
    }) if actual == pane_id && actual_time == observed_at));
    });
}

#[test]
fn osc_detection_uses_supplied_cache_and_process_exit_overrides_transcript() {
    use maestro_detection::manifest::{DetectionInput, ManifestCache};
    let cache = ManifestCache::new(None, None).unwrap();
    let transcript = "showing detailed transcript\nctrl+o to toggle";
    let input = DetectionInput {
        screen: transcript,
        osc_title: "",
        osc_progress: "",
    };
    assert!(
        detection_update_for_publish_with_osc(&cache, Some(Agent::Claude), input, false).is_none()
    );
    let exited =
        detection_update_for_publish_with_osc(&cache, Some(Agent::Claude), input, true).unwrap();
    assert_eq!(exited.state, AgentState::Idle);
    assert!(exited.visible.visible_idle);
    let active = detection_update_for_publish_with_osc(
        &cache,
        Some(Agent::Claude),
        DetectionInput {
            screen: "",
            osc_title: "⠋ Working",
            osc_progress: "",
        },
        false,
    )
    .unwrap();
    assert_eq!(active.state, AgentState::Working);
    assert!(active.visible.visible_working);
}

#[test]
fn runtime_screen_read_skips_unchanged_idle_bottom_buffer() {
    assert_eq!(
        decide_detection_screen_read(DetectionScreenReadInput {
            state: AgentState::Idle,
            agent: Some(Agent::Codex),
            pending_idle_active: false,
            agent_changed: false,
            process_exited: false,
            current_detection_content_seq: Some(10),
            last_screen_scan_detection_content_seq: Some(10),
        }),
        DetectionScreenReadDecision::Skip
    );
}

/// A transition without visible chrome.
fn publish_state(state: AgentState) -> DetectionPublishState {
    DetectionPublishState {
        state,
        visible: AgentVisibility::default(),
    }
}

#[test]
fn runtime_pending_idle_holds_working_to_plain_idle_until_confirmed() {
    let now = std::time::Instant::now();
    let mut pending = PendingIdleConfirmation::default();
    let input = DetectionTransitionInput {
        previous_publish: publish_state(AgentState::Working),
        next_publish: publish_state(AgentState::Idle),
        agent_changed: false,
        process_exited: false,
        stable_refresh_due: false,
        now,
    };
    for tick in 0..3 {
        assert_eq!(
            decide_detection_transition(
                DetectionTransitionInput {
                    now: now + AGENT_PENDING_IDLE_RECHECK * tick,
                    ..input
                },
                &mut pending
            ),
            DetectionTransitionDecision::NoPublish
        );
        assert!(pending.active());
    }
    assert_eq!(
        decide_detection_transition(
            DetectionTransitionInput {
                now: now + AGENT_PENDING_IDLE_RECHECK * 3,
                ..input
            },
            &mut pending
        ),
        DetectionTransitionDecision::PublishNext
    );
    assert!(!pending.active());
}

#[test]
fn runtime_screen_publish_keeps_visible_working_without_pty_activity() {
    let mut pending = PendingIdleConfirmation::default();
    assert_eq!(
        decide_screen_detection_publish(
            ScreenDetectionPublishInput {
                previous: publish_state(AgentState::Idle),
                last_visible_signal_refresh: None,
                screen_detection: AgentDetection {
                    state: AgentState::Working,
                    skip_state_update: false,
                    visible: AgentVisibility {
                        visible_working: true,
                        ..AgentVisibility::default()
                    },
                },
                process_exited: false,
                agent_changed: false,
                now: Instant::now(),
            },
            &mut pending
        ),
        DetectionPublishDecision::Publish {
            next: DetectionPublishState {
                state: AgentState::Working,
                visible: AgentVisibility {
                    visible_working: true,
                    ..AgentVisibility::default()
                },
            },
            process_exited: false,
        }
    );
}

#[test]
fn runtime_detection_content_change_tracks_raw_nonempty_reads_for_scan_scheduling() {
    let seq = std::sync::atomic::AtomicU64::new(0);
    observe_detection_content_change(b"", &seq);
    assert_eq!(seq.load(std::sync::atomic::Ordering::Relaxed), 0);
    observe_detection_content_change(b"\x1b[?2026h", &seq);
    assert_eq!(seq.load(std::sync::atomic::Ordering::Relaxed), 1);
    observe_detection_content_change(b"body bytes", &seq);
    assert_eq!(seq.load(std::sync::atomic::Ordering::Relaxed), 2);
}

/// An identified idle screen last read at revision ten.
fn screen_read_input(current_seq: u64) -> DetectionScreenReadInput {
    DetectionScreenReadInput {
        state: AgentState::Idle,
        agent: Some(Agent::Codex),
        pending_idle_active: false,
        agent_changed: false,
        process_exited: false,
        current_detection_content_seq: Some(current_seq),
        last_screen_scan_detection_content_seq: Some(10),
    }
}

#[test]
fn runtime_screen_read_reads_when_idle_bottom_buffer_changes() {
    assert_eq!(
        decide_detection_screen_read(screen_read_input(11)),
        DetectionScreenReadDecision::Read
    );
}

#[test]
fn runtime_screen_read_reads_for_transitions_and_missing_agent() {
    let base = screen_read_input(10);
    for input in [
        DetectionScreenReadInput {
            pending_idle_active: true,
            ..base
        },
        DetectionScreenReadInput {
            agent_changed: true,
            ..base
        },
        DetectionScreenReadInput {
            process_exited: true,
            ..base
        },
        DetectionScreenReadInput {
            agent: None,
            ..base
        },
        DetectionScreenReadInput {
            state: AgentState::Working,
            ..base
        },
        DetectionScreenReadInput {
            current_detection_content_seq: None,
            ..base
        },
        DetectionScreenReadInput {
            last_screen_scan_detection_content_seq: None,
            ..base
        },
    ] {
        assert_eq!(
            decide_detection_screen_read(input),
            DetectionScreenReadDecision::Read
        );
    }
}

/// An ordinary working-to-plain-idle observation.
fn plain_idle_transition(now: Instant) -> DetectionTransitionInput {
    DetectionTransitionInput {
        previous_publish: publish_state(AgentState::Working),
        next_publish: publish_state(AgentState::Idle),
        agent_changed: false,
        process_exited: false,
        stable_refresh_due: false,
        now,
    }
}

#[test]
fn runtime_visible_idle_bypasses_plain_idle_hold() {
    let mut pending = PendingIdleConfirmation::default();
    let mut input = plain_idle_transition(Instant::now());
    assert_eq!(
        decide_detection_transition(input, &mut pending),
        DetectionTransitionDecision::NoPublish
    );
    input.next_publish.visible.visible_idle = true;
    assert_eq!(
        decide_detection_transition(input, &mut pending),
        DetectionTransitionDecision::PublishNext
    );
    assert!(!pending.active());
}

#[test]
fn runtime_transition_decision_publishes_next_for_visible_blocker() {
    let mut pending = PendingIdleConfirmation::default();
    let mut input = plain_idle_transition(Instant::now());
    input.previous_publish = publish_state(AgentState::Idle);
    input.next_publish = publish_state(AgentState::Blocked);
    input.next_publish.visible.visible_blocker = true;
    assert_eq!(
        decide_detection_transition(input, &mut pending),
        DetectionTransitionDecision::PublishNext
    );
}

#[test]
fn runtime_screen_publish_can_publish_idle_without_input_taint_delay() {
    let mut pending = PendingIdleConfirmation::default();
    let next = DetectionPublishState {
        state: AgentState::Idle,
        visible: AgentVisibility {
            visible_idle: true,
            ..AgentVisibility::default()
        },
    };
    assert_eq!(
        decide_screen_detection_publish(
            ScreenDetectionPublishInput {
                previous: publish_state(AgentState::Blocked),
                last_visible_signal_refresh: None,
                screen_detection: AgentDetection {
                    state: next.state,
                    visible: next.visible,
                    skip_state_update: false
                },
                process_exited: false,
                agent_changed: false,
                now: Instant::now(),
            },
            &mut pending
        ),
        DetectionPublishDecision::Publish {
            next,
            process_exited: false
        }
    );
}

#[test]
fn runtime_local_terminal_mutations_can_invalidate_idle_scan_skip() {
    let seq = std::sync::atomic::AtomicU64::new(10);
    assert_eq!(
        decide_detection_screen_read(screen_read_input(10)),
        DetectionScreenReadDecision::Skip
    );
    mark_detection_content_changed(&seq);
    assert_eq!(
        decide_detection_screen_read(screen_read_input(
            seq.load(std::sync::atomic::Ordering::Relaxed)
        )),
        DetectionScreenReadDecision::Read
    );
}

#[test]
fn plain_idle_cap_and_process_changes_release_pending_confirmation() {
    let now = Instant::now();
    let input = plain_idle_transition(now);
    for release in [
        DetectionTransitionInput {
            now: now + AGENT_PENDING_IDLE_CAP,
            ..input
        },
        DetectionTransitionInput {
            agent_changed: true,
            ..input
        },
        DetectionTransitionInput {
            process_exited: true,
            ..input
        },
    ] {
        let mut pending = PendingIdleConfirmation::default();
        assert_eq!(
            decide_detection_transition(input, &mut pending),
            DetectionTransitionDecision::NoPublish
        );
        assert_eq!(
            decide_detection_transition(release, &mut pending),
            DetectionTransitionDecision::PublishNext
        );
        assert!(!pending.active());
        assert_eq!(
            decide_detection_transition(input, &mut pending),
            DetectionTransitionDecision::NoPublish
        );
    }
}

/// A published visible blocker ready for a later refresh.
fn blocker_publish_input(now: Instant) -> ScreenDetectionPublishInput {
    let blocked = DetectionPublishState {
        state: AgentState::Blocked,
        visible: AgentVisibility {
            visible_blocker: true,
            ..AgentVisibility::default()
        },
    };
    ScreenDetectionPublishInput {
        previous: blocked,
        last_visible_signal_refresh: Some(now),
        screen_detection: AgentDetection {
            state: blocked.state,
            visible: blocked.visible,
            skip_state_update: false,
        },
        process_exited: false,
        agent_changed: false,
        now,
    }
}

#[test]
fn blocker_refresh_preserves_refresh_boundary() {
    let now = Instant::now();
    let base = blocker_publish_input(now);
    let blocked = base.previous;
    let mut pending = PendingIdleConfirmation::default();
    assert_eq!(
        decide_screen_detection_publish(
            ScreenDetectionPublishInput {
                now: (now + STABLE_VISIBLE_SIGNAL_REFRESH)
                    .checked_sub(Duration::from_nanos(1))
                    .unwrap(),
                ..base
            },
            &mut pending
        ),
        DetectionPublishDecision::NoPublish
    );
    for input in [
        ScreenDetectionPublishInput {
            now: now + STABLE_VISIBLE_SIGNAL_REFRESH,
            ..base
        },
        ScreenDetectionPublishInput {
            last_visible_signal_refresh: None,
            ..base
        },
    ] {
        assert_eq!(
            decide_screen_detection_publish(input, &mut pending),
            DetectionPublishDecision::Publish {
                next: blocked,
                process_exited: false
            }
        );
    }
}

#[test]
fn screen_publication_normalizes_inconsistent_visibility() {
    let base = blocker_publish_input(Instant::now());
    let mut pending = PendingIdleConfirmation::default();
    let detection = AgentDetection {
        state: AgentState::Idle,
        skip_state_update: false,
        visible: AgentVisibility {
            visible_idle: true,
            visible_blocker: true,
            visible_working: true,
        },
    };
    let expected = DetectionPublishState {
        state: AgentState::Idle,
        visible: AgentVisibility {
            visible_idle: true,
            ..AgentVisibility::default()
        },
    };
    assert_eq!(
        decide_screen_detection_publish(
            ScreenDetectionPublishInput {
                screen_detection: detection,
                process_exited: true,
                ..base
            },
            &mut pending
        ),
        DetectionPublishDecision::Publish {
            next: expected,
            process_exited: true
        }
    );
}
