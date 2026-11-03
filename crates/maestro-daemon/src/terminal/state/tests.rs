//! Retained pane behavior tests.

use super::*;
use maestro_detection::agents::AgentDetection;

/// A terminal with a fixed identity and working directory.
pub(in crate::terminal) fn test_terminal() -> TerminalState {
    TerminalState::new(
        maestro_workspace::workspace::WorkspaceTopology::new(|| 0xabc).alloc_terminal_id(),
        "/tmp".into(),
    )
}

fn test_session_path(name: &str) -> String {
    std::env::current_dir()
        .unwrap()
        .join(name)
        .display()
        .to_string()
}

fn idle_detection(agent: Option<Agent>, process_exited: bool) -> DetectedStateUpdate {
    DetectedStateUpdate {
        agent,
        fallback_state: AgentState::Idle,
        visible: AgentVisibility {
            visible_blocker: false,
            visible_idle: true,
            visible_working: false,
        },
        process_exited,
    }
}

fn unknown_detection(agent: Option<Agent>) -> DetectedStateUpdate {
    DetectedStateUpdate {
        agent,
        fallback_state: AgentState::Unknown,
        visible: AgentVisibility {
            visible_blocker: false,
            visible_idle: false,
            visible_working: false,
        },
        process_exited: false,
    }
}

fn anchor_full_lifecycle_session(
    terminal: &mut TerminalState,
    agent: Agent,
    source: &str,
    agent_label: &str,
    session_ref: crate::agent_resume::AgentSessionRef,
) {
    terminal.set_detected_state(Some(agent), terminal.fallback_state);
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: source.into(),
        agent: agent_label.into(),
        session_ref,
    });
}

#[test]
fn pane_managed_agent_readiness_tracks_detection_state() {
    let mut terminal = test_terminal();
    let now = Instant::now();
    terminal.begin_managed_agent(
        ManagedAgentLaunch {
            name: "reviewer".into(),
            kind: Agent::Pi,
            settle_delay: Duration::from_millis(100),
            timeout: Duration::from_secs(1),
        },
        now,
    );
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Unknown);

    assert!(terminal.managed_agent_launch_pending());
    assert!(!terminal.managed_agent_interactive_ready());
    assert!(terminal.reconcile_managed_agent_at(now + Duration::from_millis(100), false));
    assert!(terminal.managed_agent_launch_pending());

    terminal.set_detected_state(Some(Agent::Pi), AgentState::Working);
    assert!(!terminal.reconcile_managed_agent_at(now + Duration::from_millis(101), false));

    terminal.set_detected_state(Some(Agent::Pi), AgentState::Blocked);
    assert!(terminal.reconcile_managed_agent_at(now + Duration::from_millis(102), false));
    assert!(terminal.managed_agent_launch_pending());
    assert!(!terminal.managed_agent_interactive_ready());
    assert_eq!(terminal.next_managed_agent_deadline(), None);
    assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));
    assert!(!terminal.reconcile_managed_agent_at(now + Duration::from_secs(2), false));
    assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));

    assert_managed_agent_stays_ready_until_exit(&mut terminal, now);
}

fn assert_managed_agent_stays_ready_until_exit(terminal: &mut TerminalState, now: Instant) {
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    assert!(terminal.reconcile_managed_agent_at(now + Duration::from_secs(2), false));
    assert!(!terminal.managed_agent_launch_pending());
    assert!(terminal.managed_agent_interactive_ready());

    terminal.set_detected_state(None, AgentState::Unknown);
    assert!(terminal.managed_agent_interactive_ready());
    assert!(!terminal.reconcile_managed_agent_at(now + Duration::from_secs(2), false));
    assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));
    assert!(terminal.reconcile_managed_agent_at(now + Duration::from_secs(2), true));
    assert_eq!(terminal.agent_name, None);
}

#[test]
fn pane_managed_agent_mismatch_and_timeout_release_name() {
    let now = Instant::now();
    let mut mismatch = test_terminal();
    mismatch.begin_managed_agent(
        ManagedAgentLaunch {
            name: "reviewer".into(),
            kind: Agent::Pi,
            settle_delay: Duration::ZERO,
            timeout: Duration::from_secs(1),
        },
        now,
    );
    mismatch.set_detected_state(Some(Agent::Codex), AgentState::Idle);
    assert!(mismatch.reconcile_managed_agent_at(now, false));
    assert_eq!(mismatch.agent_name, None);
    assert_eq!(mismatch.managed_agent_kind(), None);

    let mut timed_out = test_terminal();
    timed_out.begin_managed_agent(
        ManagedAgentLaunch {
            name: "reviewer".into(),
            kind: Agent::Pi,
            settle_delay: Duration::from_millis(10),
            timeout: Duration::from_millis(20),
        },
        now,
    );
    assert!(timed_out.reconcile_managed_agent_at(now + Duration::from_millis(20), false));
    assert_eq!(timed_out.agent_name, None);
    assert_eq!(timed_out.managed_agent_kind(), None);
}

#[test]
fn pane_stabilization_uses_raw_policy_state() {
    let detection = AgentDetection {
        state: AgentState::Idle,
        skip_state_update: false,
        visible: maestro_detection::agents::AgentVisibility {
            visible_idle: false,
            visible_blocker: false,
            visible_working: false,
        },
    };

    assert_eq!(stabilize_agent_detection(detection), AgentState::Idle);
}

#[test]
fn pane_hook_authority_overrides_fallback_for_same_agent() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("root.jsonl")).unwrap(),
    );
    terminal.set_hook_authority(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    assert_eq!(terminal.detected_agent, Some(Agent::Pi));
    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.effective_agent_label(), Some("pi"));
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_hook_authority_can_override_with_unknown_agent_label() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    terminal.set_hook_authority(HookReport {
        source: "maestro:custom".into(),
        agent_label: "custom-agent".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    assert_eq!(terminal.detected_agent, Some(Agent::Pi));
    assert_eq!(terminal.effective_agent_label(), Some("custom-agent"));
    assert_eq!(terminal.effective_known_agent(), None);
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_alternate_path_agent_hook_authority_overrides_detected_fallback() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Omp), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Omp,
        "maestro:omp",
        "omp",
        crate::agent_resume::AgentSessionRef::id("omp-root").unwrap(),
    );
    terminal.set_hook_authority(HookReport {
        source: "maestro:omp".into(),
        agent_label: "omp".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    assert_eq!(terminal.detected_agent, Some(Agent::Omp));
    assert_eq!(terminal.effective_agent_label(), Some("omp"));
    assert_eq!(terminal.effective_known_agent(), Some(Agent::Omp));
    assert_eq!(terminal.state, AgentState::Working);

    let change = terminal
        .set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent: Some(Agent::Omp),
                fallback_state: AgentState::Blocked,
                visible: AgentVisibility {
                    visible_blocker: true,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            Instant::now(),
        )
        .effective_state_change;

    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.state, AgentState::Working);
    assert!(change.is_none());
}

#[test]
fn pane_session_only_report_does_not_create_hook_authority() {
    for (agent, source, label, session_id) in [
        (Agent::Codex, "maestro:codex", "codex", "codex-session"),
        (Agent::Devin, "maestro:devin", "devin", "devin-session"),
    ] {
        let mut terminal = test_terminal();
        terminal.set_detected_state(Some(agent), AgentState::Idle);

        let mutation = terminal.set_agent_session_ref(
            source.into(),
            label.into(),
            crate::agent_resume::AgentSessionRef::id(session_id),
            Some(1),
        );

        assert!(mutation.is_some());
        assert!(terminal.hook_authority.is_none());
        assert!(!terminal.full_lifecycle_hook_authority_active());
        assert_eq!(terminal.state, AgentState::Idle);

        terminal.set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent: Some(agent),
                fallback_state: AgentState::Working,
                visible: AgentVisibility {
                    visible_blocker: false,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            Instant::now(),
        );

        assert_eq!(terminal.state, AgentState::Working);
    }
}

#[test]
fn pane_startup_session_claim_activates_full_lifecycle_integrations() {
    for (agent, source, label) in [
        (Agent::Kimi, "maestro:kimi", "kimi"),
        (Agent::Kilo, "maestro:kilo", "kilo"),
    ] {
        let mut terminal = test_terminal();
        terminal.set_detected_state(Some(agent), AgentState::Idle);
        let session_ref = crate::agent_resume::AgentSessionRef::id(format!("{label}-root"));

        let session = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
            source: source.into(),
            agent_label: label.into(),
            session_ref: session_ref.clone(),
            seq: Some(10),
            session_start_source: Some("startup".into()),
        });
        let working = terminal.set_hook_authority_with_session_ref(HookReport {
            source: source.into(),
            agent_label: label.into(),
            state: AgentState::Working,
            message: None,
            session_ref,
            seq: Some(11),
        });

        assert!(
            session.is_some(),
            "{label} should accept its startup session"
        );
        assert!(
            working.is_some(),
            "{label} should accept state after startup"
        );
        assert_eq!(terminal.state, AgentState::Working);
    }
}

#[test]
fn pane_session_identity_claims_leave_state_to_detection() {
    for (source, label, agent, start_source, replacement_source) in [
        (
            "maestro:hermes",
            "hermes",
            Agent::Hermes,
            Some("startup"),
            Some("resume"),
        ),
        (
            "maestro:antigravity_cli",
            "agy",
            Agent::Antigravity,
            None,
            None,
        ),
    ] {
        let mut terminal = test_terminal();
        assert_initial_identity_claim(&mut terminal, (source, label), agent, start_source);
        let replacement_ref = assert_detected_identity_replacement(
            &mut terminal,
            (source, label),
            agent,
            start_source,
        );
        assert_background_identity_replacement(
            &mut terminal,
            (source, label),
            agent,
            replacement_source,
            &replacement_ref,
        );
    }
}

fn assert_initial_identity_claim(
    terminal: &mut TerminalState,
    owner: (&str, &str),
    agent: Agent,
    start_source: Option<&str>,
) {
    let (source, label) = owner;
    terminal.set_detected_state(Some(agent), AgentState::Idle);
    let first_ref = crate::agent_resume::AgentSessionRef::id(format!("{label}-root")).unwrap();
    let first = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: source.into(),
        agent_label: label.into(),
        session_ref: Some(first_ref.clone()),
        seq: Some(10),
        session_start_source: start_source.map(str::to_string),
    });

    assert!(first.is_some(), "{label} should accept its session");
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.state, AgentState::Idle);
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| &session.session_ref),
        Some(&first_ref)
    );
}

fn assert_detected_identity_replacement(
    terminal: &mut TerminalState,
    owner: (&str, &str),
    agent: Agent,
    start_source: Option<&str>,
) -> crate::agent_resume::AgentSessionRef {
    let (source, label) = owner;
    terminal.set_detected_state(Some(agent), AgentState::Working);
    let replacement_ref =
        crate::agent_resume::AgentSessionRef::id(format!("{label}-replacement")).unwrap();
    let replacement = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: source.into(),
        agent_label: label.into(),
        session_ref: Some(replacement_ref.clone()),
        seq: Some(11),
        session_start_source: start_source.map(str::to_string),
    });

    assert!(
        replacement.is_some_and(|mutation| mutation.session_ref_changed),
        "{label} should replace its detected session"
    );
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| &session.session_ref),
        Some(&replacement_ref)
    );

    let legacy_state = terminal.set_hook_authority_with_session_ref(HookReport {
        source: source.into(),
        agent_label: label.into(),
        state: AgentState::Blocked,
        message: None,
        session_ref: Some(replacement_ref.clone()),
        seq: Some(12),
    });
    assert!(legacy_state.is_none());
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.state, AgentState::Working);

    replacement_ref
}

fn assert_background_identity_replacement(
    terminal: &mut TerminalState,
    owner: (&str, &str),
    agent: Agent,
    replacement_source: Option<&str>,
    replacement_ref: &crate::agent_resume::AgentSessionRef,
) {
    let (source, label) = owner;
    terminal.set_detected_state(None, AgentState::Unknown);
    let background_ref =
        crate::agent_resume::AgentSessionRef::id(format!("{label}-background")).unwrap();
    let background_replacement =
        terminal.set_agent_session_ref_for_session_start(SessionStartReport {
            source: source.into(),
            agent_label: label.into(),
            session_ref: Some(background_ref.clone()),
            seq: Some(13),
            session_start_source: replacement_source.map(str::to_string),
        });
    assert!(
        background_replacement.is_none(),
        "{label} should reject a background replacement"
    );
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| &session.session_ref),
        Some(replacement_ref)
    );

    terminal.set_detected_state(Some(agent), AgentState::Idle);
    let retried_replacement =
        terminal.set_agent_session_ref_for_session_start(SessionStartReport {
            source: source.into(),
            agent_label: label.into(),
            session_ref: Some(background_ref.clone()),
            seq: Some(14),
            session_start_source: replacement_source.map(str::to_string),
        });
    assert!(
        retried_replacement.is_some_and(|mutation| mutation.session_ref_changed),
        "{label} should replace the session once detected"
    );
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| &session.session_ref),
        Some(&background_ref)
    );
}

#[test]
fn pane_path_agent_session_replacement_reports_reanchor_full_lifecycle_authority() {
    for reason in ["new", "resume", "fork"] {
        let mut terminal = test_terminal();
        let old_session = test_session_path(&format!("pi-{reason}-old.jsonl"));
        let new_session = test_session_path(&format!("pi-{reason}-new.jsonl"));
        terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
        anchor_full_lifecycle_session(
            &mut terminal,
            Agent::Pi,
            "maestro:pi",
            "pi",
            crate::agent_resume::AgentSessionRef::path(old_session.clone()).unwrap(),
        );
        terminal
            .set_hook_authority_with_session_ref(HookReport {
                source: "maestro:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Idle,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::path(old_session),
                seq: Some(10),
            })
            .expect("initial lifecycle report must be admitted");

        let session_report = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
            seq: Some(11),
            session_start_source: Some(reason.into()),
        });

        assert!(
            session_report.is_some(),
            "{reason} should replace the previous Pi session"
        );
        assert!(terminal.hook_authority.is_none());

        let working = terminal.set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
            seq: Some(12),
        });

        assert!(
            working.is_some(),
            "{reason} should accept working for the replacement session"
        );
        assert_eq!(terminal.state, AgentState::Working);
        assert_eq!(
            terminal.hook_authority.as_ref().unwrap().session_ref,
            crate::agent_resume::AgentSessionRef::path(new_session)
        );
    }
}

#[test]
fn pane_path_agent_resume_reactivates_a_previously_stale_session() {
    let mut terminal = test_terminal();
    let session_a = test_session_path("pi-session-a.jsonl");
    let session_b = test_session_path("pi-session-b.jsonl");
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_a.clone()).unwrap(),
    );
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(session_a.clone()),
            seq: Some(10),
        })
        .expect("initial lifecycle report must be admitted");

    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::path(session_b.clone()),
            seq: Some(11),
            session_start_source: Some("new".into()),
        })
        .expect("session B must replace session A");
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(session_b.clone()),
            seq: Some(12),
        })
        .expect("session B must acquire authority");
    assert!(terminal.full_lifecycle_hook_report_matches_stale_session(
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_a.clone()).as_ref(),
    ));

    assert_path_session_reactivation(&mut terminal, session_a, session_b);
}

fn assert_path_session_reactivation(
    terminal: &mut TerminalState,
    session_a: String,
    session_b: String,
) {
    let resumed = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        session_ref: crate::agent_resume::AgentSessionRef::path(session_a.clone()),
        seq: Some(13),
        session_start_source: Some("resume".into()),
    });
    let working = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(session_a.clone()),
        seq: Some(14),
    });

    assert!(resumed.is_some());
    assert!(working.is_some());
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal.hook_authority.as_ref().unwrap().session_ref,
        crate::agent_resume::AgentSessionRef::path(session_a)
    );

    let late_session_b = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Idle,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(session_b),
        seq: Some(15),
    });
    assert!(late_session_b.is_none());
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_path_agent_startup_adopts_persisted_session_without_live_authority() {
    let mut terminal = test_terminal();
    let old_session = test_session_path("pi-startup-old.jsonl");
    let new_session = test_session_path("pi-startup-new.jsonl");
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:pi".into(),
        agent: "pi".into(),
        session_ref: crate::agent_resume::AgentSessionRef::path(old_session)
            .expect("test session path should be valid"),
    });

    let startup = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
        seq: Some(11),
        session_start_source: Some("startup".into()),
    });

    assert!(startup.is_some());
    let identity: Option<(&str, &str, &crate::agent_resume::AgentSessionRef)> =
        terminal.current_session_identity_for_persistence();
    assert_eq!(
        identity,
        Some((
            "maestro:pi",
            "pi",
            &crate::agent_resume::AgentSessionRef::path(new_session).unwrap(),
        ))
    );
}

#[test]
fn pane_path_agent_non_replacement_reports_preserve_full_lifecycle_authority() {
    for reason in [None, Some("reload"), Some("startup")] {
        let mut terminal = test_terminal();
        let old_session = test_session_path("pi-current.jsonl");
        let new_session = test_session_path("pi-unexpected.jsonl");
        terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
        anchor_full_lifecycle_session(
            &mut terminal,
            Agent::Pi,
            "maestro:pi",
            "pi",
            crate::agent_resume::AgentSessionRef::path(old_session.clone()).unwrap(),
        );
        terminal.set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(old_session.clone()),
            seq: Some(10),
        });

        let session_report = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
            seq: Some(11),
            session_start_source: reason.map(str::to_string),
        });
        let working = terminal.set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(new_session),
            seq: Some(12),
        });

        assert!(session_report.is_none());
        assert!(working.is_none());
        assert_eq!(terminal.state, AgentState::Idle);
        assert_eq!(
            terminal.hook_authority.as_ref().unwrap().session_ref,
            crate::agent_resume::AgentSessionRef::path(old_session),
            "{reason:?} must not replace the current Pi session"
        );
    }
}

#[test]
fn pane_alternate_path_agent_resume_session_report_reanchors_full_lifecycle_authority() {
    let mut terminal = test_terminal();
    let old_session = test_session_path("omp-old.jsonl");
    let new_session = test_session_path("omp-new.jsonl");
    terminal.set_detected_state(Some(Agent::Omp), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Omp,
        "maestro:omp",
        "omp",
        crate::agent_resume::AgentSessionRef::path(old_session.clone()).unwrap(),
    );
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:omp".into(),
            agent_label: "omp".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(old_session.clone()),
            seq: Some(10),
        })
        .expect("initial lifecycle report must be admitted");

    let session_report = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:omp".into(),
        agent_label: "omp".into(),
        session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
        seq: Some(11),
        session_start_source: Some("resume".into()),
    });

    assert!(session_report.is_some());
    assert!(terminal.hook_authority.is_none());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .unwrap()
            .session_ref,
        crate::agent_resume::AgentSessionRef::path(new_session.clone()).unwrap()
    );

    let blocked = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:omp".into(),
        agent_label: "omp".into(),
        state: AgentState::Blocked,
        message: Some("waiting".into()),
        session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
        seq: Some(12),
    });

    assert!(blocked.is_some());
    assert_eq!(terminal.state, AgentState::Blocked);
    assert_eq!(
        terminal.hook_authority.as_ref().unwrap().session_ref,
        crate::agent_resume::AgentSessionRef::path(new_session)
    );

    assert_alternate_path_rejects_previous_session(&mut terminal, old_session);
}

fn assert_alternate_path_rejects_previous_session(
    terminal: &mut TerminalState,
    old_session: String,
) {
    let stale = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:omp".into(),
        agent_label: "omp".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(old_session),
        seq: Some(13),
    });

    assert!(stale.is_none());
    assert_eq!(terminal.state, AgentState::Blocked);
}

#[test]
fn pane_late_full_lifecycle_hook_with_same_session_after_process_exit_does_not_reacquire_authority()
{
    let now = Instant::now();
    let mut terminal = test_terminal();
    let session_path = test_session_path("pi.jsonl");
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Working);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_path.clone()).unwrap(),
    );
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(session_path.clone()),
            seq: Some(20),
        })
        .expect("initial lifecycle report must be admitted");

    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), true),
        now + Duration::from_millis(1),
    );
    let late = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(session_path),
        seq: Some(21),
    });

    assert!(late.is_none());
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.state, AgentState::Idle);
}

#[test]
fn pane_live_full_lifecycle_hook_rejects_different_session_ref_for_same_source() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("one.jsonl")).unwrap(),
    );
    terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(test_session_path("one.jsonl")),
        seq: Some(20),
    });

    let mutation = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Idle,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(test_session_path("two.jsonl")),
        seq: Some(21),
    });

    assert!(mutation.is_none());
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal
            .hook_authority
            .as_ref()
            .and_then(|authority| authority.session_ref.as_ref())
            .map(|session_ref| session_ref.value.as_str()),
        Some(test_session_path("one.jsonl").as_str())
    );
}

#[test]
fn pane_fresh_detected_process_keeps_old_session_suppressed_after_process_exit() {
    let mut terminal = test_terminal();
    let old_session = test_session_path("old-process-exit.jsonl");
    let new_session = test_session_path("new-process-exit.jsonl");
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(old_session.clone()).unwrap(),
    );
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(old_session.clone()),
            seq: Some(1000),
        })
        .expect("initial lifecycle report must be admitted");
    let process_exit_seen_at = Instant::now() + Duration::from_secs(1);
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), true),
        process_exit_seen_at,
    );

    let fresh_process_seen_at = process_exit_seen_at + Duration::from_millis(1);
    terminal
        .set_detected_state_with_screen_signals_at(unknown_detection(None), fresh_process_seen_at);
    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(Some(Agent::Pi)),
        fresh_process_seen_at + Duration::from_millis(1),
    );

    assert_fresh_path_session_requires_startup(&mut terminal, old_session, new_session);
}

fn assert_fresh_path_session_requires_startup(
    terminal: &mut TerminalState,
    old_session: String,
    new_session: String,
) {
    let late_old = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(old_session),
        seq: Some(500),
    });
    let fresh_new = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
        seq: Some(501),
    });

    assert!(late_old.is_none());
    assert!(fresh_new.is_none());
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::path(new_session),
            seq: Some(400),
            session_start_source: Some("startup".into()),
        })
        .expect("fresh session should activate the buffered report");
    assert!(terminal.hook_authority.is_some());
    assert_eq!(terminal.state, AgentState::Working);
}

fn start_then_exit_path_agent(terminal: &mut TerminalState, session_path: &str, now: Instant) {
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_path).unwrap(),
    );
    terminal
        .set_hook_authority_at(
            HookReport {
                source: "maestro:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::path(session_path),
                seq: Some(1000),
            },
            now,
        )
        .expect("initial lifecycle report must be admitted");
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), true),
        now + Duration::from_millis(1),
    );
}

#[test]
fn pane_rapid_restart_replays_reports_that_arrive_before_process_evidence() {
    let mut terminal = test_terminal();
    let session_path = test_session_path("reports-before-process-evidence.jsonl");
    let now = Instant::now();
    start_then_exit_path_agent(&mut terminal, &session_path, now);

    let lower_sequence = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(session_path.clone()),
            seq: Some(1001),
        },
        now + Duration::from_millis(2),
    );
    let missing_sequence = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(session_path.clone()),
            seq: None,
        },
        now + Duration::from_millis(3),
    );
    let buffered_working = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(session_path.clone()),
            seq: Some(2001),
        },
        now + Duration::from_millis(4),
    );
    let startup = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        session_ref: crate::agent_resume::AgentSessionRef::path(session_path),
        seq: Some(2000),
        session_start_source: Some("startup".into()),
    });
    assert!(startup.is_none());
    assert!(lower_sequence.is_none());
    assert!(missing_sequence.is_none());
    assert!(buffered_working.is_none());
    assert!(!terminal.full_lifecycle_hook_authority_active());

    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), false),
        now + Duration::from_millis(5),
    );

    assert!(terminal.full_lifecycle_hook_authority_active());
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_process_exit_discards_unclaimed_buffered_state_from_that_generation() {
    let mut terminal = test_terminal();
    let old_session = test_session_path("buffered-exit-old.jsonl");
    let shared_session = test_session_path("buffered-exit-shared.jsonl");
    let now = Instant::now();
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(old_session.clone()).unwrap(),
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(old_session),
            seq: Some(1000),
        },
        now,
    );
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), true),
        now + Duration::from_millis(1),
    );
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), false),
        now + Duration::from_millis(2),
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(shared_session.clone()),
            seq: Some(500),
        },
        now + Duration::from_millis(3),
    );

    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), true),
        now + Duration::from_millis(4),
    );
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), false),
        now + Duration::from_millis(5),
    );
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::path(shared_session),
            seq: Some(100),
            session_start_source: Some("startup".into()),
        })
        .expect("new generation session claim");

    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.state, AgentState::Idle);
}

#[test]
fn pane_queued_fresh_process_evidence_uses_process_exit_observation_time() {
    let mut terminal = test_terminal();
    let session_path = test_session_path("queued-after-process-exit.jsonl");
    let process_exit_at = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_path.clone()).unwrap(),
    );
    terminal
        .set_hook_authority_at(
            HookReport {
                source: "maestro:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::path(session_path.clone()),
                seq: Some(1000),
            },
            process_exit_at
                .checked_sub(Duration::from_millis(1))
                .unwrap(),
        )
        .expect("initial lifecycle report must be admitted");
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), true),
        process_exit_at,
    );

    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(None),
        process_exit_at + Duration::from_millis(1),
    );
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), false),
        process_exit_at + Duration::from_millis(2),
    );
    let startup = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        session_ref: crate::agent_resume::AgentSessionRef::path(session_path),
        seq: Some(2000),
        session_start_source: Some("startup".into()),
    });

    assert!(startup.is_some());
}

#[test]
fn pane_different_session_after_process_exit_waits_for_fresh_process_evidence() {
    let mut terminal = test_terminal();
    let old_session = test_session_path("old-before-process-exit.jsonl");
    let new_session = test_session_path("new-after-process-exit.jsonl");
    let now = Instant::now();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(old_session.clone()).unwrap(),
    );
    terminal
        .set_hook_authority_at(
            HookReport {
                source: "maestro:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::path(old_session),
                seq: Some(1000),
            },
            now,
        )
        .expect("initial lifecycle report must be admitted");
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Pi), true),
        now + Duration::from_millis(1),
    );

    let early_new = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(new_session.clone()),
            seq: Some(500),
        },
        now + Duration::from_millis(2),
    );

    assert!(early_new.is_none());
    assert!(terminal.hook_authority.is_none());

    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(None),
        now + Duration::from_millis(3),
    );
    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(Some(Agent::Pi)),
        now + Duration::from_millis(4),
    );
    let fresh_new = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        session_ref: crate::agent_resume::AgentSessionRef::path(new_session),
        seq: Some(400),
        session_start_source: Some("startup".into()),
    });

    assert!(fresh_new.is_some());
    assert!(terminal.hook_authority.is_some());
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_missing_session_after_process_exit_waits_for_fresh_process_evidence() {
    let mut terminal = test_terminal();
    let old_session = test_session_path("old-before-nosession-process-exit.jsonl");
    let now = Instant::now();
    start_then_exit_path_agent(&mut terminal, &old_session, now);

    let early_without_session = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(500),
        },
        now + Duration::from_millis(2),
    );

    assert!(early_without_session.is_none());
    assert!(terminal.hook_authority.is_none());

    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(None),
        now + Duration::from_millis(3),
    );
    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(Some(Agent::Pi)),
        now + Duration::from_millis(4),
    );
    let fresh_without_session = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(500),
        },
        now + Duration::from_millis(5),
    );
    assert!(fresh_without_session.is_none());
    assert!(terminal.hook_authority.is_none());

    assert_root_claim_allows_sessionless_child(&mut terminal, now);
}

fn assert_root_claim_allows_sessionless_child(terminal: &mut TerminalState, now: Instant) {
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::path(test_session_path(
                "fresh-after-nosession-process-exit.jsonl",
            )),
            seq: Some(600),
            session_start_source: Some("startup".into()),
        })
        .expect("fresh root session should claim the process generation");
    let child_update = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(601),
        },
        now + Duration::from_millis(6),
    );

    assert!(child_update.is_some());
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_thread_agent_session_start_replaces_current_root_session() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Mastracode), AgentState::Idle);
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:mastracode".into(),
            agent_label: "mastracode".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("mastracode-old"),
            seq: Some(20),
            session_start_source: Some("startup".into()),
        })
        .expect("initial root session");

    let replacement = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:mastracode".into(),
        agent_label: "mastracode".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("mastracode-new"),
        seq: Some(21),
        session_start_source: Some("startup".into()),
    });

    assert!(replacement.is_some_and(|mutation| mutation.session_ref_changed));
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("mastracode-new")
    );
}

#[test]
fn pane_alternate_path_agent_reacquires_full_lifecycle_hook_after_process_exit_with_fresh_process_and_session_ref()
 {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Omp), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Omp,
        "maestro:omp",
        "omp",
        crate::agent_resume::AgentSessionRef::id("omp-old").unwrap(),
    );
    terminal
        .set_hook_authority_at(
            HookReport {
                source: "maestro:omp".into(),
                agent_label: "omp".into(),
                state: AgentState::Working,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::id("omp-old"),
                seq: Some(1000),
            },
            now,
        )
        .expect("initial lifecycle report must be admitted");
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Omp), true),
        now + Duration::from_millis(1),
    );

    let stale = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:omp".into(),
        agent_label: "omp".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::id("omp-old"),
        seq: Some(500),
    });
    assert!(stale.is_none());
    assert!(terminal.hook_authority.is_none());

    assert_alternate_path_reacquires_fresh_session(&mut terminal, now);
}

fn assert_alternate_path_reacquires_fresh_session(terminal: &mut TerminalState, now: Instant) {
    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(None),
        now + Duration::from_millis(2),
    );
    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(Some(Agent::Omp)),
        now + Duration::from_millis(3),
    );
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:omp".into(),
            agent_label: "omp".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("omp-new"),
            seq: Some(400),
            session_start_source: Some("startup".into()),
        })
        .expect("fresh process and session should claim the pane");
    let fresh = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:omp".into(),
        agent_label: "omp".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::id("omp-new"),
        seq: Some(500),
    });

    assert!(fresh.is_some());
    assert!(terminal.hook_authority.is_some());
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_visible_blocker_overrides_non_blocked_hook_for_same_agent() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Idle);
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    let change = terminal
        .set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent: Some(Agent::Codex),
                fallback_state: AgentState::Blocked,
                visible: AgentVisibility {
                    visible_blocker: true,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            Instant::now(),
        )
        .effective_state_change;

    assert_eq!(terminal.fallback_state, AgentState::Blocked);
    assert_eq!(terminal.state, AgentState::Blocked);
    assert_eq!(change.unwrap().previous_state, AgentState::Working);
}

#[test]
fn pane_visible_blocker_does_not_override_full_lifecycle_hook_authority() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("root.jsonl")).unwrap(),
    );
    terminal.set_hook_authority(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    let change = terminal
        .set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent: Some(Agent::Pi),
                fallback_state: AgentState::Blocked,
                visible: AgentVisibility {
                    visible_blocker: true,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            Instant::now(),
        )
        .effective_state_change;

    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.state, AgentState::Working);
    assert!(change.is_none());
}

#[test]
fn pane_weak_blocked_fallback_does_not_override_hook_authority() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Idle);
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    let change = terminal
        .set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent: Some(Agent::Codex),
                fallback_state: AgentState::Blocked,
                visible: AgentVisibility {
                    visible_blocker: false,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            Instant::now(),
        )
        .effective_state_change;

    assert_eq!(terminal.fallback_state, AgentState::Blocked);
    assert_eq!(terminal.state, AgentState::Working);
    assert!(change.is_none());
}

#[test]
fn pane_hook_blocked_wins_over_visible_blocker() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Working);
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Blocked,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Blocked,
            visible: AgentVisibility {
                visible_blocker: true,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        Instant::now(),
    );

    assert_eq!(terminal.state, AgentState::Blocked);
    assert!(terminal.hook_authority.is_some());
}

#[test]
fn pane_visible_blocker_does_not_override_different_agent_hook() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(None, AgentState::Unknown);
    terminal.set_hook_authority(HookReport {
        source: "custom:agent".into(),
        agent_label: "custom-agent".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Blocked,
            visible: AgentVisibility {
                visible_blocker: true,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        Instant::now(),
    );

    assert_eq!(terminal.effective_agent_label(), Some("custom-agent"));
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_fallback_idle_does_not_override_hook_working() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Claude), AgentState::Working);
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:claude".into(),
            agent_label: "claude".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: None,
        },
        now,
    );

    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Claude), false),
        now + Duration::from_secs(10),
    );

    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_fallback_idle_does_not_override_full_lifecycle_hook_working() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Working);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::OpenCode,
        "maestro:opencode",
        "opencode",
        crate::agent_resume::AgentSessionRef::id("opencode-root").unwrap(),
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: None,
        },
        now,
    );
    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::OpenCode), false),
        now + Duration::from_secs(10),
    );

    assert_eq!(terminal.fallback_state, AgentState::Working);
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_visible_working_does_not_override_hook_idle_for_same_agent() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Claude), AgentState::Idle);
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:claude".into(),
            agent_label: "claude".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: None,
            seq: None,
        },
        now,
    );

    let change = terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Claude),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: true,
            },
            process_exited: false,
        },
        now + Duration::from_millis(1),
    );

    assert_eq!(terminal.fallback_state, AgentState::Working);
    assert_eq!(terminal.state, AgentState::Idle);
    assert!(change.effective_state_change.is_none());
}

#[test]
fn pane_visible_working_does_not_override_full_lifecycle_hook_idle() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Kimi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Kimi,
        "maestro:kimi",
        "kimi",
        crate::agent_resume::AgentSessionRef::id("kimi-root").unwrap(),
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:kimi".into(),
            agent_label: "kimi".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: None,
            seq: None,
        },
        now,
    );

    let change = terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Kimi),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: true,
            },
            process_exited: false,
        },
        now + Duration::from_millis(1),
    );

    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.state, AgentState::Idle);
    assert!(change.effective_state_change.is_none());
}

#[test]
fn pane_detected_working_fallback_is_ignored_under_full_lifecycle_hook_authority() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Kilo), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Kilo,
        "maestro:kilo",
        "kilo",
        crate::agent_resume::AgentSessionRef::id("kilo-root").unwrap(),
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:kilo".into(),
            agent_label: "kilo".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: None,
            seq: None,
        },
        now,
    );

    let change = terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Kilo),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        now + Duration::from_millis(1),
    );

    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.state, AgentState::Idle);
    assert!(change.effective_state_change.is_none());
}

#[test]
fn pane_visible_working_does_not_hold_against_newer_prompt_agent_hook_idle() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Claude),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: true,
            },
            process_exited: false,
        },
        now,
    );

    let change = terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:claude".into(),
            agent_label: "claude".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: None,
            seq: None,
        },
        now + Duration::from_millis(100),
    );

    assert_eq!(terminal.state, AgentState::Idle);
    assert_eq!(
        change
            .unwrap()
            .effective_state_change
            .unwrap()
            .previous_state,
        AgentState::Working
    );
}

#[test]
fn pane_refreshed_visible_working_does_not_override_newer_hook_blocked() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: true,
            },
            process_exited: false,
        },
        now,
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:codex".into(),
            agent_label: "codex".into(),
            state: AgentState::Blocked,
            message: None,
            session_ref: None,
            seq: None,
        },
        now + Duration::from_millis(1201),
    );

    assert_eq!(terminal.state, AgentState::Blocked);

    let change = terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: true,
            },
            process_exited: false,
        },
        now + Duration::from_millis(2000),
    );

    assert_eq!(terminal.fallback_state, AgentState::Working);
    assert_eq!(terminal.state, AgentState::Blocked);
    assert!(change.effective_state_change.is_none());
}

#[test]
fn pane_fallback_idle_does_not_override_other_agent_hook_working() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Working);
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    let change = terminal
        .set_detected_state_with_screen_signals_at(
            DetectedStateUpdate {
                agent: Some(Agent::Codex),
                fallback_state: AgentState::Idle,
                visible: AgentVisibility {
                    visible_blocker: false,
                    visible_idle: false,
                    visible_working: false,
                },
                process_exited: false,
            },
            Instant::now(),
        )
        .effective_state_change;

    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.state, AgentState::Working);
    assert!(change.is_none());
}

#[test]
fn pane_known_hook_authority_does_not_override_different_detected_agent() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Grok), AgentState::Working);
    let change = terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Blocked,
        message: None,
        session_ref: None,
        seq: None,
    });

    assert!(change.is_none());
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent, Some(Agent::Grok));
    assert_eq!(terminal.effective_agent_label(), Some("grok"));
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_detected_agent_clears_conflicting_known_hook_authority() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Blocked,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state(Some(Agent::Grok), AgentState::Working);

    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent, Some(Agent::Grok));
    assert_eq!(terminal.effective_agent_label(), Some("grok"));
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_border_label_prefers_manual_label_over_agent_label() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Claude), AgentState::Idle);

    assert_eq!(terminal.border_label(false), None);
    assert_eq!(terminal.border_label(true).as_deref(), Some("claude"));

    terminal.set_manual_label(" reviewer ");
    assert_eq!(terminal.border_label(false).as_deref(), Some("reviewer"));
    assert_eq!(terminal.border_label(true).as_deref(), Some("reviewer"));

    terminal.set_manual_label("   ");
    assert_eq!(terminal.border_label(true).as_deref(), Some("claude"));

    terminal.set_manual_label("reviewer");
    terminal.clear_manual_label();
    assert_eq!(terminal.border_label(true).as_deref(), Some("claude"));
}

#[test]
fn pane_hook_authority_survives_unrelated_detected_agent_clear() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    terminal.set_hook_authority(HookReport {
        source: "maestro:custom".into(),
        agent_label: "custom-agent".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state(None, AgentState::Unknown);

    assert!(terminal.hook_authority.is_some());
    assert_eq!(terminal.detected_agent, None);
    assert_eq!(terminal.effective_agent_label(), Some("custom-agent"));
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_full_lifecycle_hook_authority_ignores_detected_agent_clear_without_process_exit() {
    let now = Instant::now();
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("root.jsonl")).unwrap(),
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: None,
        },
        now,
    );

    let change = terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(None),
        now + Duration::from_millis(1),
    );

    assert!(terminal.hook_authority.is_some());
    assert_eq!(terminal.detected_agent, Some(Agent::Pi));
    assert_eq!(terminal.fallback_state, AgentState::Idle);
    assert_eq!(terminal.state, AgentState::Working);
    assert!(change.effective_state_change.is_none());
}

#[test]
fn pane_detected_agent_clear_clears_matching_hook_authority() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Cursor), AgentState::Idle);
    terminal.set_hook_authority(HookReport {
        source: "maestro:cursor".into(),
        agent_label: "cursor".into(),
        state: AgentState::Idle,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state(None, AgentState::Unknown);

    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent, None);
    assert_eq!(terminal.fallback_state, AgentState::Unknown);
    assert_eq!(terminal.effective_agent_label(), None);
    assert_eq!(terminal.state, AgentState::Unknown);
}

#[test]
fn pane_detected_agent_clear_clears_matching_working_hook_authority() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Working);
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state(None, AgentState::Unknown);

    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent, None);
    assert_eq!(terminal.effective_agent_label(), None);
    assert_eq!(terminal.state, AgentState::Unknown);
}

#[test]
fn pane_process_exit_clears_matching_hook_authority_before_reporting_idle() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Working);
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: true,
        },
        Instant::now(),
    );

    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent, Some(Agent::Codex));
    assert_eq!(terminal.effective_agent_label(), None);
    assert_eq!(terminal.state, AgentState::Idle);
}

#[test]
fn pane_stale_visible_screen_signal_does_not_override_newer_hook_authority() {
    let mut terminal = test_terminal();
    let observed = Instant::now();
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Claude),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: true,
            },
            process_exited: false,
        },
        observed,
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:claude".into(),
            agent_label: "claude".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(1),
        },
        observed + Duration::from_secs(1),
    );

    terminal.set_detected_state_with_screen_signals_at(
        idle_detection(Some(Agent::Claude), false),
        observed,
    );

    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_stale_process_exit_preserves_newer_custom_authority() {
    let mut terminal = test_terminal();
    let observed = Instant::now();
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        observed,
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "custom:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(100),
        },
        observed + Duration::from_secs(1),
    );

    let mutation = terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: true,
        },
        observed,
    );

    assert!(!mutation.agent_released);
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal
            .hook_authority
            .as_ref()
            .map(|hook| hook.source.as_str()),
        Some("custom:pi")
    );
}

#[test]
fn pane_custom_authority_reanchors_sequence_after_process_restart() {
    let mut terminal = test_terminal();
    let observed = Instant::now();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    terminal.set_hook_authority_at(
        HookReport {
            source: "custom:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(100),
        },
        observed,
    );
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: true,
        },
        observed + Duration::from_millis(1),
    );
    terminal.set_detected_state_with_screen_signals_at(
        unknown_detection(None),
        observed + Duration::from_millis(2),
    );

    assert_custom_authority_reanchors_with_process(&mut terminal, observed);
}

fn assert_custom_authority_reanchors_with_process(terminal: &mut TerminalState, observed: Instant) {
    assert!(
        terminal
            .release_agent_with_mutation("custom:pi", "pi", Some(200))
            .is_none()
    );
    assert!(
        terminal
            .clear_hook_authority_with_mutation(Some("custom:pi"), Some(201))
            .is_none()
    );
    assert!(
        terminal
            .set_hook_authority(HookReport {
                source: "custom:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: None,
                seq: Some(1)
            })
            .is_none()
    );
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        observed + Duration::from_millis(3),
    );
    assert!(
        terminal
            .set_hook_authority(HookReport {
                source: "custom:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: None,
                seq: Some(1)
            })
            .is_some()
    );
}

#[test]
fn pane_process_exit_clears_newer_same_agent_hook_authority() {
    let mut terminal = test_terminal();
    let observed = Instant::now();
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        observed,
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:codex".into(),
            agent_label: "codex".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(1),
        },
        observed,
    );
    terminal.set_hook_authority_at(
        HookReport {
            source: "maestro:codex".into(),
            agent_label: "codex".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(2),
        },
        observed + Duration::from_secs(1),
    );

    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: true,
        },
        observed,
    );

    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.state, AgentState::Idle);
    assert_eq!(terminal.effective_agent_label(), None);
}

#[test]
fn pane_detected_agent_change_clears_previous_matching_hook_authority() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Idle);
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Idle,
        message: None,
        session_ref: None,
        seq: None,
    });

    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Working);

    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent, Some(Agent::OpenCode));
    assert_eq!(terminal.effective_agent_label(), Some("opencode"));
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_stale_hook_report_sequence_is_ignored_for_same_source() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("root.jsonl")).unwrap(),
    );
    terminal.set_hook_authority(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: Some(20),
    });

    let change = terminal.set_hook_authority(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Idle,
        message: None,
        session_ref: None,
        seq: Some(19),
    });

    assert!(change.is_none());
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal.hook_authority.as_ref().unwrap().state,
        AgentState::Working
    );
}

#[test]
fn pane_accepted_hook_report_stores_session_ref() {
    let mut terminal = test_terminal();
    let session_path = test_session_path("pi.jsonl");
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_path.clone()).unwrap(),
    );
    let mutation = terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::path(session_path.clone()),
            seq: Some(20),
        })
        .expect("accepted report");

    assert!(!mutation.session_ref_changed);
    assert_eq!(
        terminal
            .hook_authority
            .as_ref()
            .and_then(|authority| authority.session_ref.as_ref())
            .map(|session_ref| (&session_ref.kind, session_ref.value.as_str())),
        Some((
            &crate::agent_resume::AgentSessionRefKind::Path,
            session_path.as_str()
        ))
    );
}

#[test]
fn pane_stale_hook_report_cannot_overwrite_session_ref() {
    let mut terminal = test_terminal();
    let session_path = test_session_path("pi.jsonl");
    let new_session_path = test_session_path("new.jsonl");
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_path.clone()).unwrap(),
    );
    terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(session_path.clone()),
        seq: Some(20),
    });

    let mutation = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(new_session_path),
        seq: Some(19),
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal
            .hook_authority
            .as_ref()
            .and_then(|authority| authority.session_ref.as_ref())
            .map(|session_ref| session_ref.value.as_str()),
        Some(session_path.as_str())
    );
}

#[test]
fn pane_accepted_hook_report_without_session_ref_clears_previous_ref() {
    let mut terminal = test_terminal();
    let session_path = test_session_path("pi.jsonl");
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_path.clone()).unwrap(),
    );
    terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(session_path),
        seq: Some(20),
    });

    let mutation = terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(21),
        })
        .expect("accepted report");

    assert!(mutation.session_ref_changed);
    assert!(mutation.effective_state_change.is_none());
    assert!(
        terminal
            .hook_authority
            .as_ref()
            .unwrap()
            .session_ref
            .is_none()
    );
}

#[test]
fn pane_different_same_agent_session_ref_is_ignored_until_current_session_clears() {
    let mut terminal = test_terminal();
    terminal
        .set_agent_session_ref(
            "maestro:claude".into(),
            "claude".into(),
            crate::agent_resume::AgentSessionRef::id("claude-session"),
            Some(20),
        )
        .expect("initial session should be accepted");

    let mutation = terminal.set_agent_session_ref(
        "maestro:claude".into(),
        "claude".into(),
        crate::agent_resume::AgentSessionRef::id("nested-session"),
        Some(21),
    );

    assert!(mutation.is_none());
    assert_eq!(
        terminal.hook_report_sequences.get("maestro:claude"),
        Some(&21)
    );
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("claude-session")
    );
}

#[test]
fn pane_prompt_agent_startup_session_ref_does_not_replace_existing_session_ref() {
    let mut terminal = test_terminal();
    terminal
        .set_agent_session_ref(
            "maestro:claude".into(),
            "claude".into(),
            crate::agent_resume::AgentSessionRef::id("claude-session"),
            Some(20),
        )
        .expect("initial session should be accepted");

    let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("nested-session"),
        seq: Some(21),
        session_start_source: Some("startup".into()),
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("claude-session")
    );
}

#[test]
fn pane_prompt_agent_lifecycle_session_ref_replaces_existing_session_ref() {
    for session_start_source in ["clear", "resume", "compact"] {
        let mut terminal = test_terminal();
        terminal
            .set_agent_session_ref(
                "maestro:claude".into(),
                "claude".into(),
                crate::agent_resume::AgentSessionRef::id("claude-session"),
                Some(20),
            )
            .expect("initial session should be accepted");

        let next_session = format!("{session_start_source}-session");
        let mutation = terminal
            .set_agent_session_ref_for_session_start(SessionStartReport {
                source: "maestro:claude".into(),
                agent_label: "claude".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id(&next_session),
                seq: Some(21),
                session_start_source: Some(session_start_source.into()),
            })
            .unwrap_or_else(|| panic!("{session_start_source} should replace the session"));

        assert!(
            mutation.session_ref_changed,
            "{session_start_source} should mark the session changed"
        );
        assert_eq!(
            terminal
                .persisted_agent_session
                .as_ref()
                .map(|session| session.session_ref.value.as_str()),
            Some(next_session.as_str()),
            "{session_start_source} should store the replacement session"
        );
    }
}

#[test]
fn pane_code_agent_lifecycle_session_ref_replaces_existing_session_ref() {
    for session_start_source in ["startup", "clear", "resume", "compact"] {
        let mut terminal = test_terminal();
        terminal
            .set_agent_session_ref(
                "maestro:codex".into(),
                "codex".into(),
                crate::agent_resume::AgentSessionRef::id("codex-session"),
                Some(20),
            )
            .expect("initial session should be accepted");

        let next_session = format!("codex-{session_start_source}-session");
        let mutation = terminal
            .set_agent_session_ref_for_session_start(SessionStartReport {
                source: "maestro:codex".into(),
                agent_label: "codex".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id(&next_session),
                seq: Some(21),
                session_start_source: Some(session_start_source.into()),
            })
            .unwrap_or_else(|| panic!("{session_start_source} should replace the session"));

        assert!(mutation.session_ref_changed);
        assert_eq!(
            terminal
                .persisted_agent_session
                .as_ref()
                .map(|session| session.session_ref.value.as_str()),
            Some(next_session.as_str())
        );
    }
}

#[test]
fn pane_branch_agent_lifecycle_session_ref_replaces_existing_session_ref() {
    for session_start_source in ["startup", "clear", "resume", "compact", "branch"] {
        let mut terminal = test_terminal();
        terminal.set_detected_state(Some(Agent::Qwen), AgentState::Idle);
        terminal
            .set_agent_session_ref(
                "maestro:qwen".into(),
                "qwen".into(),
                crate::agent_resume::AgentSessionRef::id("qwen-session"),
                Some(20),
            )
            .expect("initial session should be accepted");

        let next_session = format!("qwen-{session_start_source}-session");
        let mutation = terminal
            .set_agent_session_ref_for_session_start(SessionStartReport {
                source: "maestro:qwen".into(),
                agent_label: "qwen".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id(&next_session),
                seq: Some(21),
                session_start_source: Some(session_start_source.into()),
            })
            .unwrap_or_else(|| panic!("{session_start_source} should replace the session"));

        assert!(mutation.session_ref_changed);
        assert_eq!(
            terminal
                .persisted_agent_session
                .as_ref()
                .map(|session| session.session_ref.value.as_str()),
            Some(next_session.as_str())
        );
    }
}

#[test]
fn pane_branch_agent_session_ref_does_not_replace_without_foreground_branch_agent() {
    let mut terminal = test_terminal();
    terminal
        .set_agent_session_ref(
            "maestro:qwen".into(),
            "qwen".into(),
            crate::agent_resume::AgentSessionRef::id("qwen-parent"),
            Some(20),
        )
        .expect("initial session should be accepted");

    let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:qwen".into(),
        agent_label: "qwen".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("qwen-branch"),
        seq: Some(21),
        session_start_source: Some("branch".into()),
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("qwen-parent")
    );
}

#[test]
fn pane_selection_agent_server_new_does_not_replace_existing_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Idle);
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("opencode-visible"),
            seq: None,
            session_start_source: Some("select".into()),
        })
        .expect("local selection should be accepted");

    let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("opencode-attached-client"),
        seq: Some(21),
        session_start_source: Some("new".into()),
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("opencode-visible")
    );
}

#[test]
fn pane_selection_agent_server_resume_does_not_replace_existing_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Idle);
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("opencode-visible"),
            seq: None,
            session_start_source: Some("select".into()),
        })
        .expect("local selection should be accepted");

    let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("opencode-attached-client"),
        seq: Some(21),
        session_start_source: Some("resume".into()),
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("opencode-visible")
    );
}

#[test]
fn pane_selection_agent_tui_selection_anchors_after_process_detection() {
    let mut terminal = test_terminal();
    let startup_selection = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("opencode-startup-selection"),
        seq: None,
        session_start_source: Some("select".into()),
    });
    assert!(startup_selection.is_none());
    assert_eq!(
        terminal
            .suppressed_full_lifecycle_hook_reports
            .get("maestro:opencode")
            .and_then(|suppressed| suppressed.replacement_session_ref.as_ref())
            .map(|session| session.value.as_str()),
        Some("opencode-startup-selection")
    );

    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Idle);
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("opencode-startup-selection")
    );
    assert!(
        !terminal
            .suppressed_full_lifecycle_hook_reports
            .contains_key("maestro:opencode")
    );

    assert_selection_reconciles_suppressed_generation(&mut terminal);
}

fn assert_selection_reconciles_suppressed_generation(terminal: &mut TerminalState) {
    terminal.suppressed_full_lifecycle_hook_reports.insert(
        "maestro:opencode".into(),
        SuppressedFullLifecycleHookReport {
            agent_label: "opencode".into(),
            session_ref: None,
            observed_at: Instant::now(),
            reason: FullLifecycleHookSuppressionReason::ProcessExit,
            replacement_session_ref: None,
            pending_replacement_report: None,
        },
    );
    let selected = terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("opencode-reselected"),
            seq: None,
            session_start_source: Some("select".into()),
        })
        .expect("local TUI selection should reconcile generation suppression");

    assert!(selected.session_ref_changed);
    assert!(
        !terminal
            .suppressed_full_lifecycle_hook_reports
            .contains_key("maestro:opencode")
    );
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("opencode-reselected")
    );
    assert!(
        !terminal
            .hook_report_sequences
            .contains_key("maestro:opencode")
    );
}

#[test]
fn pane_selection_agent_tui_selection_reanchors_full_lifecycle_authority() {
    let mut terminal = test_terminal();
    let old_session = crate::agent_resume::AgentSessionRef::id("opencode-newer").unwrap();
    let selected_session =
        crate::agent_resume::AgentSessionRef::id("opencode-selected-older").unwrap();
    let attached_session = assert_selection_ignores_attached_session(&mut terminal, &old_session);
    assert_selection_reanchors_authority(&mut terminal, &selected_session);
    assert_selection_rejects_stale_sessions(
        &mut terminal,
        old_session,
        attached_session,
        &selected_session,
    );
    assert_subsequent_selection_replaces_session(&mut terminal);
}

fn assert_selection_ignores_attached_session(
    terminal: &mut TerminalState,
    old_session: &crate::agent_resume::AgentSessionRef,
) -> crate::agent_resume::AgentSessionRef {
    anchor_full_lifecycle_session(
        terminal,
        Agent::OpenCode,
        "maestro:opencode",
        "opencode",
        old_session.clone(),
    );
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: Some(old_session.clone()),
            seq: Some(20),
        })
        .expect("initial session should own lifecycle state");
    let attached_session =
        crate::agent_resume::AgentSessionRef::id("opencode-attached-client").unwrap();
    let attached = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        state: AgentState::Working,
        message: None,
        session_ref: Some(attached_session.clone()),
        seq: Some(21),
    });
    assert!(attached.is_none());
    assert!(
        !terminal
            .suppressed_full_lifecycle_hook_reports
            .contains_key("maestro:opencode")
    );

    attached_session
}

fn assert_selection_reanchors_authority(
    terminal: &mut TerminalState,
    selected_session: &crate::agent_resume::AgentSessionRef,
) {
    let selected = terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            session_ref: Some(selected_session.clone()),
            seq: None,
            session_start_source: Some("select".into()),
        })
        .expect("selected session should replace the previous session");

    assert!(selected.session_ref_changed);
    assert!(terminal.hook_authority.is_none());
    assert!(
        !terminal
            .suppressed_full_lifecycle_hook_reports
            .contains_key("maestro:opencode")
    );
    assert_eq!(
        terminal.hook_report_sequences.get("maestro:opencode"),
        Some(&20)
    );
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| &session.session_ref),
        Some(selected_session)
    );

    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            state: AgentState::Working,
            message: None,
            session_ref: Some(selected_session.clone()),
            seq: Some(21),
        })
        .expect("selected session should regain lifecycle authority");
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal
            .hook_authority
            .as_ref()
            .and_then(|authority| authority.session_ref.as_ref()),
        Some(selected_session)
    );
}

fn assert_selection_rejects_stale_sessions(
    terminal: &mut TerminalState,
    old_session: crate::agent_resume::AgentSessionRef,
    attached_session: crate::agent_resume::AgentSessionRef,
    selected_session: &crate::agent_resume::AgentSessionRef,
) {
    let late_old_session = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        state: AgentState::Idle,
        message: None,
        session_ref: Some(old_session),
        seq: Some(22),
    });
    assert!(late_old_session.is_none());
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal
            .hook_authority
            .as_ref()
            .and_then(|authority| authority.session_ref.as_ref()),
        Some(selected_session)
    );

    let late_attached_session = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        state: AgentState::Blocked,
        message: None,
        session_ref: Some(attached_session),
        seq: Some(23),
    });
    assert!(late_attached_session.is_none());
    assert_eq!(terminal.state, AgentState::Working);
}

fn assert_subsequent_selection_replaces_session(terminal: &mut TerminalState) {
    let final_session =
        crate::agent_resume::AgentSessionRef::id("opencode-final-selection").unwrap();
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            session_ref: Some(final_session.clone()),
            seq: None,
            session_start_source: Some("select".into()),
        })
        .expect("another local selection should remain authoritative");
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| &session.session_ref),
        Some(&final_session)
    );
    assert!(
        !terminal
            .suppressed_full_lifecycle_hook_reports
            .contains_key("maestro:opencode")
    );
}

#[test]
fn pane_path_agent_session_replacement_clears_the_previous_sessions_alias() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("pi-old"),
            seq: Some(20),
            session_start_source: Some("new".into()),
        })
        .expect("initial session should be accepted");
    terminal.set_agent_name("reviewer".into());

    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("pi-new"),
            seq: Some(21),
            session_start_source: Some("new".into()),
        })
        .expect("new should replace the session");

    assert!(terminal.agent_name.is_none());
}

#[test]
fn pane_managed_agent_name_survives_native_session_replacement() {
    let mut terminal = test_terminal();
    let now = Instant::now();
    terminal.begin_managed_agent(
        ManagedAgentLaunch {
            name: "reviewer".into(),
            kind: Agent::OpenCode,
            settle_delay: Duration::ZERO,
            timeout: Duration::from_secs(1),
        },
        now,
    );
    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Idle);
    assert!(terminal.reconcile_managed_agent_at(now, false));

    for session in ["opencode-old", "opencode-new"] {
        terminal
            .set_agent_session_ref_for_session_start(SessionStartReport {
                source: "maestro:opencode".into(),
                agent_label: "opencode".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id(session),
                seq: None,
                session_start_source: Some("select".into()),
            })
            .expect("managed session should be accepted");
    }

    assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));
    assert!(terminal.managed_agent_interactive_ready());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("opencode-new")
    );
}

#[test]
fn pane_selection_agent_session_ref_without_start_source_does_not_replace_existing() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Idle);
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("opencode-old"),
            seq: None,
            session_start_source: Some("select".into()),
        })
        .expect("local selection should be accepted");

    // session.updated reports carry no session_start_source, so a different
    // id must not displace the established session (cross-talk guard).
    let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("opencode-other"),
        seq: Some(21),
        session_start_source: None,
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("opencode-old")
    );
}

#[test]
fn pane_different_owner_session_ref_does_not_replace_existing_session_ref() {
    let mut terminal = test_terminal();
    terminal
        .set_agent_session_ref(
            "maestro:droid".into(),
            "droid".into(),
            crate::agent_resume::AgentSessionRef::id("droid-session"),
            Some(20),
        )
        .expect("initial session should be accepted");

    let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("claude-session"),
        seq: Some(21),
        session_start_source: Some("resume".into()),
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal.persisted_agent_session.as_ref().map(|session| (
            session.source.as_str(),
            session.agent.as_str(),
            session.session_ref.value.as_str()
        )),
        Some(("maestro:droid", "droid", "droid-session"))
    );
}

#[test]
fn pane_foreground_agent_session_replaces_stale_different_owner_session_ref() {
    for session_start_source in ["resume", "startup"] {
        let mut terminal = test_terminal();
        terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
            source: "maestro:codex".into(),
            agent: "codex".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("codex-session").unwrap(),
        });
        terminal.set_detected_state(Some(Agent::Claude), AgentState::Idle);

        let mutation = terminal
            .set_agent_session_ref_for_session_start(SessionStartReport {
                source: "maestro:claude".into(),
                agent_label: "claude".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id("claude-session"),
                seq: Some(21),
                session_start_source: Some(session_start_source.into()),
            })
            .unwrap_or_else(|| panic!("{session_start_source} should replace stale codex session"));

        assert!(mutation.session_ref_changed);
        assert_eq!(
            terminal.persisted_agent_session.as_ref().map(|session| (
                session.source.as_str(),
                session.agent.as_str(),
                session.session_ref.value.as_str()
            )),
            Some(("maestro:claude", "claude", "claude-session")),
            "{session_start_source} should store claude session"
        );
    }
}

#[test]
fn pane_foreground_agent_session_requires_lifecycle_source_to_replace_different_owner() {
    for session_start_source in [None, Some("other")] {
        let mut terminal = test_terminal();
        terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
            source: "maestro:codex".into(),
            agent: "codex".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("codex-session").unwrap(),
        });
        terminal.set_detected_state(Some(Agent::Claude), AgentState::Idle);

        let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:claude".into(),
            agent_label: "claude".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("claude-session"),
            seq: Some(21),
            session_start_source: session_start_source.map(str::to_string),
        });

        assert!(
            mutation.is_none(),
            "{session_start_source:?} should not replace"
        );
        assert_eq!(
            terminal.persisted_agent_session.as_ref().map(|session| (
                session.source.as_str(),
                session.agent.as_str(),
                session.session_ref.value.as_str()
            )),
            Some(("maestro:codex", "codex", "codex-session"))
        );
    }
}

#[test]
fn pane_different_owner_session_ref_requires_matching_detected_agent() {
    for session_start_source in ["startup", "resume"] {
        for detected_agent in [None, Some(Agent::Codex)] {
            let mut terminal = test_terminal();
            terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
                source: "maestro:codex".into(),
                agent: "codex".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id("codex-session").unwrap(),
            });
            terminal.set_detected_state(detected_agent, AgentState::Idle);

            let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
                source: "maestro:claude".into(),
                agent_label: "claude".into(),
                session_ref: crate::agent_resume::AgentSessionRef::id("claude-session"),
                seq: Some(21),
                session_start_source: Some(session_start_source.into()),
            });

            assert!(
                mutation.is_none(),
                "{session_start_source} with {detected_agent:?} should not replace"
            );
            assert_eq!(
                terminal.persisted_agent_session.as_ref().map(|session| (
                    session.source.as_str(),
                    session.agent.as_str(),
                    session.session_ref.value.as_str()
                )),
                Some(("maestro:codex", "codex", "codex-session"))
            );
        }
    }
}

#[test]
fn pane_custom_session_report_does_not_replace_different_owner_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:codex".into(),
        agent: "codex".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("codex-session").unwrap(),
    });
    terminal.set_detected_state(Some(Agent::Claude), AgentState::Idle);

    let mutation = terminal.set_agent_session_ref_for_session_start(SessionStartReport {
        source: "custom:claude".into(),
        agent_label: "claude".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("claude-session"),
        seq: Some(21),
        session_start_source: Some("resume".into()),
    });

    assert!(mutation.is_none());
    assert_eq!(
        terminal.persisted_agent_session.as_ref().map(|session| (
            session.source.as_str(),
            session.agent.as_str(),
            session.session_ref.value.as_str()
        )),
        Some(("maestro:codex", "codex", "codex-session"))
    );
}

#[test]
fn pane_foreground_agent_session_replaces_stale_different_owner_hook_authority() {
    let mut terminal = test_terminal();
    let now = std::time::Instant::now();
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::OpenCode,
        "maestro:opencode",
        "opencode",
        crate::agent_resume::AgentSessionRef::id("opencode-session").unwrap(),
    );
    terminal
        .set_hook_authority_at(
            HookReport {
                source: "maestro:opencode".into(),
                agent_label: "opencode".into(),
                state: AgentState::Working,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::id("opencode-session"),
                seq: Some(20),
            },
            now + Duration::from_millis(1),
        )
        .expect("initial hook authority should be accepted");
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Codex),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        now,
    );

    let mutation = terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:codex".into(),
            agent_label: "codex".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("codex-session"),
            seq: Some(21),
            session_start_source: Some("startup".into()),
        })
        .expect("foreground codex should replace stale hook authority");

    assert!(mutation.session_ref_changed);
    assert!(terminal.hook_authority.is_none());
    assert_eq!(
        terminal.current_session_identity_for_persistence(),
        Some((
            "maestro:codex",
            "codex",
            &crate::agent_resume::AgentSessionRef::id("codex-session").unwrap()
        ))
    );
    assert_old_owner_requires_fresh_selection(&mut terminal);
}

fn assert_old_owner_requires_fresh_selection(terminal: &mut TerminalState) {
    let late_old_session = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::id("opencode-session"),
        seq: Some(22),
    });
    assert!(late_old_session.is_none());

    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Idle);
    terminal
        .set_agent_session_ref_for_session_start(SessionStartReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            session_ref: crate::agent_resume::AgentSessionRef::id("opencode-new-session"),
            seq: None,
            session_start_source: Some("select".into()),
        })
        .expect("fresh local selection");
    let fresh_session = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::id("opencode-new-session"),
        seq: Some(24),
    });
    assert!(fresh_session.is_some());
}

#[test]
fn pane_different_owner_full_lifecycle_hook_does_not_replace_existing_session_ref() {
    let mut terminal = test_terminal();
    terminal
        .set_agent_session_ref(
            "maestro:droid".into(),
            "droid".into(),
            crate::agent_resume::AgentSessionRef::id("droid-session"),
            Some(20),
        )
        .expect("initial session should be accepted");

    let mutation = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path("/tmp/pi-session.jsonl"),
        seq: Some(21),
    });

    assert!(mutation.is_none());
    assert!(terminal.hook_authority.is_none());
    assert_eq!(
        terminal.persisted_agent_session.as_ref().map(|session| (
            session.source.as_str(),
            session.agent.as_str(),
            session.session_ref.value.as_str()
        )),
        Some(("maestro:droid", "droid", "droid-session"))
    );
}

#[test]
fn pane_repeated_same_agent_session_ref_is_accepted_without_session_change() {
    let mut terminal = test_terminal();
    terminal
        .set_agent_session_ref(
            "maestro:claude".into(),
            "claude".into(),
            crate::agent_resume::AgentSessionRef::id("claude-session"),
            Some(20),
        )
        .expect("initial session should be accepted");

    let mutation = terminal
        .set_agent_session_ref(
            "maestro:claude".into(),
            "claude".into(),
            crate::agent_resume::AgentSessionRef::id("claude-session"),
            Some(21),
        )
        .expect("same session should be accepted");

    assert!(!mutation.session_ref_changed);
}

#[test]
fn pane_hook_authority_rejects_state_from_a_different_session() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::OpenCode), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::OpenCode,
        "maestro:opencode",
        "opencode",
        crate::agent_resume::AgentSessionRef::id("opencode-session").unwrap(),
    );
    terminal
        .set_hook_authority_with_session_ref(HookReport {
            source: "maestro:opencode".into(),
            agent_label: "opencode".into(),
            state: AgentState::Working,
            message: None,
            session_ref: crate::agent_resume::AgentSessionRef::id("opencode-session"),
            seq: Some(20),
        })
        .expect("initial session should be accepted");

    let mutation = terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:opencode".into(),
        agent_label: "opencode".into(),
        state: AgentState::Blocked,
        message: Some("needs approval".into()),
        session_ref: crate::agent_resume::AgentSessionRef::id("nested-session"),
        seq: Some(21),
    });

    assert!(mutation.is_none());
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(
        terminal
            .hook_authority
            .as_ref()
            .and_then(|authority| authority.session_ref.as_ref())
            .map(|session_ref| session_ref.value.as_str()),
        Some("opencode-session")
    );
}

#[test]
fn pane_detected_agent_clear_does_not_clear_current_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Claude), AgentState::Working);
    terminal
        .set_agent_session_ref(
            "maestro:claude".into(),
            "claude".into(),
            crate::agent_resume::AgentSessionRef::id("claude-session"),
            Some(20),
        )
        .expect("initial session should be accepted");

    let clear = terminal.set_detected_state_with_mutation(None, AgentState::Unknown);
    assert!(!clear.session_ref_changed);

    let mutation = terminal.set_agent_session_ref(
        "maestro:claude".into(),
        "claude".into(),
        crate::agent_resume::AgentSessionRef::id("new-session"),
        Some(21),
    );

    assert!(mutation.is_none());
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("claude-session")
    );
}

#[test]
fn pane_clearing_hook_authority_clears_session_ref() {
    let mut terminal = test_terminal();
    let session_path = test_session_path("pi.jsonl");
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(session_path.clone()).unwrap(),
    );
    terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::path(session_path),
        seq: Some(20),
    });

    let mutation = terminal
        .clear_hook_authority_with_mutation(Some("maestro:pi"), Some(21))
        .expect("accepted clear");

    assert!(mutation.session_ref_changed);
    assert!(terminal.hook_authority.is_none());
}

#[test]
fn pane_agent_alias_survives_detection_uncertainty_and_reported_release_but_not_replacement() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Working);
    terminal.set_agent_name("reviewer".into());

    terminal.set_detected_state(None, AgentState::Unknown);
    assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));

    terminal.set_detected_state(Some(Agent::Codex), AgentState::Idle);
    assert!(terminal.agent_name.is_none());

    terminal.set_agent_name("replacement".into());
    let mutation = terminal
        .release_agent_with_mutation("maestro:codex", "codex", None)
        .expect("detected agent release should be accepted");
    assert!(!mutation.agent_released);
    assert_eq!(terminal.agent_name.as_deref(), Some("replacement"));
    assert_eq!(terminal.detected_agent, Some(Agent::Codex));
}

#[test]
fn pane_custom_release_preserves_process_owned_agent_state() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    terminal
        .set_hook_authority(HookReport {
            source: "custom:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(10),
        })
        .expect("custom state should be accepted");

    let mutation = terminal
        .release_agent_with_mutation("custom:pi", "pi", Some(11))
        .expect("custom release should be accepted");

    assert!(!mutation.agent_released);
    assert!(terminal.hook_authority.is_none());
    assert_eq!(terminal.detected_agent, Some(Agent::Pi));
    assert_eq!(terminal.effective_agent_label(), Some("pi"));
    assert_eq!(terminal.state, AgentState::Idle);
}

#[test]
fn pane_agent_replacement_clears_alias_owned_by_hook_identity() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: Some(20),
    });
    terminal.set_agent_name("reviewer".into());

    terminal.set_detected_state(Some(Agent::Grok), AgentState::Idle);

    assert!(terminal.agent_name.is_none());
    assert_eq!(terminal.effective_known_agent(), Some(Agent::Grok));
}

#[test]
fn pane_accepted_hook_replacement_clears_the_previous_agents_alias() {
    let mut terminal = test_terminal();
    terminal
        .set_hook_authority_at(
            HookReport {
                source: "custom:agent".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: None,
                seq: Some(20),
            },
            Instant::now(),
        )
        .expect("initial hook should be accepted");
    terminal.set_agent_name("reviewer".into());

    terminal
        .set_hook_authority_at(
            HookReport {
                source: "custom:agent".into(),
                agent_label: "claude".into(),
                state: AgentState::Idle,
                message: None,
                session_ref: None,
                seq: Some(21),
            },
            Instant::now(),
        )
        .expect("replacement hook should be accepted");

    assert!(terminal.agent_name.is_none());
    assert_eq!(terminal.effective_known_agent(), Some(Agent::Claude));
}

#[test]
fn pane_accepted_same_kind_hook_owner_replacement_clears_the_alias() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("first.jsonl")).unwrap(),
    );
    terminal
        .set_hook_authority_at(
            HookReport {
                source: "maestro:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Working,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::path(test_session_path(
                    "first.jsonl",
                )),
                seq: Some(20),
            },
            Instant::now(),
        )
        .expect("initial hook should be accepted");
    terminal.set_agent_name("reviewer".into());
    terminal
        .clear_hook_authority_with_mutation(Some("maestro:pi"), Some(21))
        .expect("hook clear should be accepted");
    assert_eq!(terminal.agent_name.as_deref(), Some("reviewer"));

    terminal
        .set_hook_authority_at(
            HookReport {
                source: "maestro:pi".into(),
                agent_label: "pi".into(),
                state: AgentState::Idle,
                message: None,
                session_ref: crate::agent_resume::AgentSessionRef::path(test_session_path(
                    "second.jsonl",
                )),
                seq: Some(22),
            },
            Instant::now(),
        )
        .expect("replacement hook should be accepted");

    assert!(terminal.agent_name.is_none());
    assert_eq!(terminal.effective_known_agent(), Some(Agent::Pi));
}

#[test]
fn pane_launch_command_alone_does_not_make_a_terminal_an_agent() {
    let terminal = test_terminal().with_launch_argv(vec!["just".into(), "dev".into()]);

    assert!(!terminal.is_agent_terminal());
}

#[test]
fn pane_release_agent_clears_matching_restored_session_ref_before_detection() {
    let mut terminal = test_terminal();
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:hermes".into(),
        agent: "hermes".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("hermes-session").unwrap(),
    });

    let mutation = terminal
        .release_agent_with_mutation("maestro:hermes", "hermes", Some(21))
        .expect("accepted release");

    assert!(mutation.session_ref_changed);
    assert!(mutation.effective_state_change.is_none());
    assert!(terminal.persisted_agent_session.is_none());
}

#[test]
fn pane_release_agent_preserves_foreign_persisted_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:claude".into(),
        agent: "claude".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("claude-session").unwrap(),
    });
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);

    let mutation = terminal
        .release_agent_with_mutation("maestro:pi", "pi", Some(21))
        .expect("visible agent release should be accepted");

    assert!(!mutation.session_ref_changed);
    assert_eq!(
        terminal.persisted_agent_session.as_ref().map(|session| (
            session.source.as_str(),
            session.agent.as_str(),
            session.session_ref.value.as_str()
        )),
        Some(("maestro:claude", "claude", "claude-session"))
    );
}

#[test]
fn pane_process_exit_clears_matching_persisted_session_ref() {
    let mut terminal = test_terminal();
    let session_ref =
        crate::agent_resume::AgentSessionRef::path(test_session_path("pi.jsonl")).unwrap();
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:pi".into(),
        agent: "pi".into(),
        session_ref: session_ref.clone(),
    });
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Working);

    let mutation = terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: true,
        },
        std::time::Instant::now(),
    );

    assert!(mutation.session_ref_changed);
    assert!(terminal.persisted_agent_session.is_none());

    let delayed = terminal.set_agent_session_ref(
        "maestro:pi".into(),
        "pi".into(),
        Some(session_ref),
        Some(21),
    );
    assert!(delayed.is_none());
    assert!(terminal.persisted_agent_session.is_none());
}

#[test]
fn pane_process_exit_preserves_foreign_persisted_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:claude".into(),
        agent: "claude".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("claude-session").unwrap(),
    });
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Working);

    let mutation = terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: true,
        },
        std::time::Instant::now(),
    );

    assert!(!mutation.session_ref_changed);
    assert_eq!(
        terminal
            .persisted_agent_session
            .as_ref()
            .map(|session| session.session_ref.value.as_str()),
        Some("claude-session")
    );
}

#[test]
fn pane_respawn_cleanup_resets_restored_agent_status() {
    let mut terminal = test_terminal();
    terminal.respawn_shell_on_exit = true;
    terminal.set_agent_name("codex".into());
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:codex".into(),
        agent: "codex".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("codex-session").unwrap(),
    });
    terminal.set_detected_state(Some(Agent::Codex), AgentState::Idle);
    terminal.set_detected_agent_process_at(Agent::Codex, Instant::now());

    terminal.clear_agent_runtime_identity_after_respawn();

    assert_eq!(terminal.state, AgentState::Unknown);
    assert!(terminal.detected_agent.is_none());
    assert!(terminal.agent_name.is_none());
    assert!(terminal.persisted_agent_session.is_none());
    assert!(!terminal.respawn_shell_on_exit);
    assert!(!terminal.finish_agent_process_acquisition());
}

#[test]
fn pane_agent_process_exit_tracks_recent_respawn_window() {
    let mut terminal = test_terminal();
    let now = std::time::Instant::now();

    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::OpenCode),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: true,
        },
        now,
    );

    assert!(terminal.agent_process_exited_within(now, Duration::from_secs(2)));
    assert!(
        !terminal.agent_process_exited_within(now + Duration::from_secs(3), Duration::from_secs(2))
    );

    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::OpenCode),
            fallback_state: AgentState::Working,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: true,
            },
            process_exited: false,
        },
        now + Duration::from_secs(4),
    );

    assert!(
        !terminal.agent_process_exited_within(now + Duration::from_secs(4), Duration::from_secs(2))
    );
}

#[test]
fn pane_detected_conflict_clears_live_hook_but_preserves_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::id("claude-session"),
        seq: Some(20),
    });

    let mutation = terminal.set_detected_state_with_mutation(Some(Agent::Grok), AgentState::Idle);

    assert!(!mutation.session_ref_changed);
    assert!(terminal.hook_authority.is_none());
    assert_eq!(
        terminal.persisted_agent_session.as_ref().map(|session| (
            session.source.as_str(),
            session.agent.as_str(),
            session.session_ref.value.as_str()
        )),
        Some(("maestro:claude", "claude", "claude-session"))
    );
}

#[test]
fn pane_detected_agent_disappearance_does_not_clear_full_lifecycle_hook_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Kimi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Kimi,
        "maestro:kimi",
        "kimi",
        crate::agent_resume::AgentSessionRef::id("kimi-session").unwrap(),
    );
    terminal.set_hook_authority_with_session_ref(HookReport {
        source: "maestro:kimi".into(),
        agent_label: "kimi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: crate::agent_resume::AgentSessionRef::id("kimi-session"),
        seq: Some(20),
    });

    let mutation = terminal.set_detected_state_with_mutation(None, AgentState::Unknown);

    assert!(!mutation.session_ref_changed);
    assert!(terminal.hook_authority.is_some());
    assert!(terminal.persisted_agent_session.is_none());
    assert_eq!(terminal.effective_agent_label(), Some("kimi"));
}

#[test]
fn pane_detected_agent_disappearance_preserves_matching_persisted_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:opencode".into(),
        agent: "opencode".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("opencode-session").unwrap(),
    });

    let first = terminal.set_detected_state_with_mutation(Some(Agent::OpenCode), AgentState::Idle);
    assert!(!first.session_ref_changed);
    assert!(terminal.persisted_agent_session.is_some());

    let second = terminal.set_detected_state_with_mutation(None, AgentState::Unknown);
    assert!(!second.session_ref_changed);
    assert!(terminal.persisted_agent_session.is_some());
}

#[test]
fn pane_initial_unknown_detection_preserves_restored_session_ref() {
    let mut terminal = test_terminal();
    terminal.set_persisted_agent_session(crate::agent_resume::PersistedAgentSession {
        source: "maestro:hermes".into(),
        agent: "hermes".into(),
        session_ref: crate::agent_resume::AgentSessionRef::id("hermes-session").unwrap(),
    });

    let mutation = terminal.set_detected_state_with_mutation(None, AgentState::Unknown);
    assert!(!mutation.session_ref_changed);
    assert!(terminal.persisted_agent_session.is_some());
}

#[test]
fn pane_unsequenced_hook_report_is_ignored_after_source_uses_sequence() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("root.jsonl")).unwrap(),
    );
    terminal.set_hook_authority(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: Some(20),
    });

    let change = terminal.set_hook_authority(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Idle,
        message: None,
        session_ref: None,
        seq: None,
    });

    assert!(change.is_none());
    assert_eq!(terminal.state, AgentState::Working);
}

#[test]
fn pane_stale_clear_all_sequence_is_checked_against_current_authority_source() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::path(test_session_path("root.jsonl")).unwrap(),
    );
    terminal.set_hook_authority(HookReport {
        source: "maestro:pi".into(),
        agent_label: "pi".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: Some(20),
    });

    let change = terminal.clear_hook_authority(None, Some(19));

    assert!(change.is_none());
    assert_eq!(terminal.state, AgentState::Working);
    assert!(terminal.hook_authority.is_some());
}

#[test]
fn pane_same_sequence_from_different_sources_is_independent() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    anchor_full_lifecycle_session(
        &mut terminal,
        Agent::Pi,
        "maestro:pi",
        "pi",
        crate::agent_resume::AgentSessionRef::id("sequence-owner").unwrap(),
    );
    terminal
        .set_hook_authority(HookReport {
            source: "maestro:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: None,
            seq: Some(20),
        })
        .expect("initial lifecycle report must be admitted");

    assert_eq!(terminal.hook_report_sequences.get("maestro:pi"), Some(&20));
    terminal
        .set_hook_authority(HookReport {
            source: "custom:pi".into(),
            agent_label: "pi".into(),
            state: AgentState::Idle,
            message: None,
            session_ref: None,
            seq: Some(20),
        })
        .expect("equal sequence from a different source must be admitted");
    assert_eq!(terminal.hook_report_sequences.get("custom:pi"), Some(&20));

    assert_eq!(terminal.state, AgentState::Idle);
    assert_eq!(
        terminal.hook_authority.as_ref().unwrap().source,
        "custom:pi"
    );
}

mod boundaries;
