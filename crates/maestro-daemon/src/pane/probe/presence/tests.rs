use super::*;
#[test]
fn identity_shell_return_reports_process_exit_before_clearing_agent() {
    assert_eq!(
        foreground_shell_agent_action(ForegroundShellObservation {
            previous_agent: Some(Agent::Codex),
            new_agent: None,
            foreground_is_pane_shell: true,
            process_exit_reported: false
        }),
        ForegroundShellAgentAction::ReportProcessExit
    );
    assert_eq!(
        foreground_shell_agent_action(ForegroundShellObservation {
            previous_agent: Some(Agent::Codex),
            new_agent: None,
            foreground_is_pane_shell: true,
            process_exit_reported: true
        }),
        ForegroundShellAgentAction::ClearAgent
    );
}

#[test]
fn identity_same_agent_after_reported_exit_is_a_replacement_process() {
    assert_eq!(
        foreground_shell_agent_action(ForegroundShellObservation {
            previous_agent: Some(Agent::Pi),
            new_agent: Some(Agent::Pi),
            foreground_is_pane_shell: false,
            process_exit_reported: true
        }),
        ForegroundShellAgentAction::ReportReplacementProcess
    );
}

#[test]
fn identity_unknown_non_shell_foreground_job_is_not_immediate_clear_signal() {
    assert_eq!(
        foreground_shell_agent_action(ForegroundShellObservation {
            previous_agent: Some(Agent::Claude),
            new_agent: None,
            foreground_is_pane_shell: false,
            process_exit_reported: false
        }),
        ForegroundShellAgentAction::ObserveProbe
    );
}

#[test]
fn identity_reported_process_exit_clears_before_unknown_foreground_probe() {
    assert_eq!(
        foreground_shell_agent_action(ForegroundShellObservation {
            previous_agent: Some(Agent::Claude),
            new_agent: None,
            foreground_is_pane_shell: false,
            process_exit_reported: true
        }),
        ForegroundShellAgentAction::ClearAgent
    );
}

#[test]
fn identity_foreground_agent_job_is_not_clear_signal() {
    assert_eq!(
        foreground_shell_agent_action(ForegroundShellObservation {
            previous_agent: Some(Agent::Claude),
            new_agent: Some(Agent::OpenCode),
            foreground_is_pane_shell: true,
            process_exit_reported: false
        }),
        ForegroundShellAgentAction::ObserveProbe
    );
}

#[test]
fn identity_transient_scan_miss_keeps_current_agent_detected() {
    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Pi));

    let changed = presence.observe_process_probe(None);

    assert!(!changed, "one miss should not clear the detected agent");
    assert_eq!(presence.current_agent(), Some(Agent::Pi));
}

#[test]
fn identity_clears_after_six_misses() {
    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Pi));

    for attempt in 1..AGENT_MISS_CONFIRMATION_ATTEMPTS {
        let changed = presence.observe_process_probe(None);
        assert!(
            !changed,
            "miss {attempt} should stay in the confirmation window"
        );
        assert_eq!(presence.current_agent(), Some(Agent::Pi));
    }

    let changed = presence.observe_process_probe(None);
    assert!(changed, "last confirmation miss should clear the agent");
    assert_eq!(presence.current_agent(), None);
}

#[test]
fn shell_return_publishes_completion_before_release_and_replacement() {
    let mut presence = AgentDetectionPresence::from_agent(Some(Agent::Codex));
    let mut state = ForegroundShellState::default();
    let returned = ForegroundShellObservation {
        previous_agent: Some(Agent::Codex),
        new_agent: None,
        foreground_is_pane_shell: true,
        process_exit_reported: false,
    };
    assert!(!state.apply(&mut presence, returned));
    assert!(state.pending_clear);
    assert_eq!(presence.current_agent(), Some(Agent::Codex));
    state.exit_reported = true;
    assert!(state.apply(
        &mut presence,
        ForegroundShellObservation {
            process_exit_reported: state.exit_reported,
            ..returned
        }
    ));
    assert_eq!(presence.current_agent(), None);
    assert!(!state.pending_clear);
    assert!(!state.exit_reported);

    let identified = ForegroundShellObservation {
        previous_agent: None,
        new_agent: Some(Agent::Codex),
        ..returned
    };
    assert!(state.apply(&mut presence, identified));
    assert!(!state.apply(
        &mut presence,
        ForegroundShellObservation {
            previous_agent: Some(Agent::Codex),
            ..identified
        }
    ));
    assert!(state.apply(
        &mut presence,
        ForegroundShellObservation {
            previous_agent: Some(Agent::Codex),
            process_exit_reported: true,
            ..identified
        }
    ));
    assert_eq!(presence.current_agent(), Some(Agent::Codex));
    assert!(state.apply(
        &mut presence,
        ForegroundShellObservation {
            previous_agent: Some(Agent::Codex),
            new_agent: Some(Agent::Claude),
            process_exit_reported: true,
            ..returned
        }
    ));
    assert_eq!(presence.current_agent(), Some(Agent::Claude));
}

#[test]
fn successful_scan_resets_confirmation_misses() {
    let mut presence = AgentDetectionPresence::from_agent(None);
    assert!(!presence.observe_process_probe(None));
    assert!(!presence.clear_current_agent());
    assert!(presence.observe_process_probe(Some(Agent::Codex)));
    for _ in 1..AGENT_MISS_CONFIRMATION_ATTEMPTS {
        assert!(!presence.observe_process_probe(None));
    }
    assert!(!presence.observe_process_probe(Some(Agent::Codex)));
    assert!(!presence.observe_process_probe(None));
    assert_eq!(presence.current_agent(), Some(Agent::Codex));
    assert!(presence.clear_current_agent());
    assert!(!presence.clear_current_agent());
}
