use super::*;
use maestro_detection::agents::Agent;
fn process_probe_input() -> ProcessProbeInput {
    ProcessProbeInput {
        current_agent: None,
        suppressed_agent: None,
        foreground_pgid: Some(42),
        last_foreground_pgid: Some(42),
        has_process_probe: true,
        acquisition_age: None,
        pending_foreground_shell_clear: false,
        pending_restore_probe: false,
        elapsed_since_process_check: std::time::Duration::from_secs(1),
    }
}

#[test]
fn probe_policy_windows_foreground_observation_schedule_preserves_lifecycle_checks() {
    let quiet = ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        ..process_probe_input()
    };
    let before_safety_bound = PROCESS_RECHECK_IDENTIFIED
        .checked_sub(std::time::Duration::from_millis(1))
        .unwrap();
    let observe = |lifecycle, content_changed, elapsed, input| {
        should_observe_foreground_process_group(
            input,
            ForegroundObservation {
                lifecycle_authority: lifecycle,
                content_changed,
                elapsed,
            },
        )
    };
    assert!(!observe(false, false, before_safety_bound, quiet));
    assert!(observe(false, true, before_safety_bound, quiet));
    assert!(observe(true, true, before_safety_bound, quiet));
    assert!(observe(
        false,
        false,
        before_safety_bound,
        ProcessProbeInput {
            elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED,
            ..quiet
        }
    ));
    assert!(observe(true, false, PROCESS_RECHECK_IDENTIFIED, quiet));

    for immediate in [
        ProcessProbeInput {
            has_process_probe: false,
            ..quiet
        },
        ProcessProbeInput {
            current_agent: None,
            acquisition_age: Some(std::time::Duration::ZERO),
            ..quiet
        },
        ProcessProbeInput {
            pending_restore_probe: true,
            ..quiet
        },
        ProcessProbeInput {
            suppressed_agent: Some(Agent::Codex),
            ..quiet
        },
        ProcessProbeInput {
            pending_foreground_shell_clear: true,
            ..quiet
        },
    ] {
        assert!(observe(false, false, std::time::Duration::ZERO, immediate));
    }
}

#[test]
fn probe_policy_unchanged_unknown_job_group_skips_full_scan() {
    assert!(!should_probe_foreground_job(process_probe_input()));
}

#[test]
fn probe_policy_unknown_job_group_change_runs_full_scan() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        foreground_pgid: Some(43),
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_unknown_pane_gets_initial_scan() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        has_process_probe: false,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_stable_unknown_job_group_has_no_safety_scan() {
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        elapsed_since_process_check: PROCESS_RECHECK_MISSING_FOREGROUND_GROUP,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_unknown_pane_without_job_group_uses_safety_scan() {
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        foreground_pgid: None,
        last_foreground_pgid: None,
        ..process_probe_input()
    }));
    assert!(should_probe_foreground_job(ProcessProbeInput {
        foreground_pgid: None,
        last_foreground_pgid: None,
        elapsed_since_process_check: PROCESS_RECHECK_MISSING_FOREGROUND_GROUP,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_unknown_pane_probes_when_job_group_disappears() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        foreground_pgid: None,
        last_foreground_pgid: Some(42),
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_inferred_group_does_not_trigger_a_probe_on_every_tick() {
    let tracked = process_group_for_change_tracking(None, Some(300));
    assert_eq!(tracked, None);
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Claude),
        foreground_pgid: None,
        last_foreground_pgid: tracked,
        elapsed_since_process_check: std::time::Duration::from_millis(300),
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_pending_shell_clear_and_restore_force_scans() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        pending_foreground_shell_clear: true,
        ..process_probe_input()
    }));
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        pending_restore_probe: true,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_lifecycle_owner_skips_stable_routine_scan() {
    assert!(should_skip_process_probe_for_lifecycle_authority(
        true,
        ProcessProbeInput {
            current_agent: Some(Agent::Pi),
            elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED,
            ..process_probe_input()
        }
    ));
    assert!(!should_skip_process_probe_for_lifecycle_authority(
        false,
        ProcessProbeInput {
            current_agent: Some(Agent::Pi),
            elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED,
            ..process_probe_input()
        }
    ));
}

#[test]
fn probe_policy_lifecycle_owner_keeps_periodic_probes_without_an_observed_group() {
    let input = ProcessProbeInput {
        current_agent: Some(Agent::Pi),
        foreground_pgid: None,
        last_foreground_pgid: None,
        elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED,
        ..process_probe_input()
    };
    assert!(!should_skip_process_probe_for_lifecycle_authority(
        true, input
    ));
    assert!(should_probe_foreground_job(input));
}

#[test]
fn probe_policy_lifecycle_owner_preserves_process_exit_and_release_probes() {
    assert!(!should_skip_process_probe_for_lifecycle_authority(
        true,
        ProcessProbeInput {
            current_agent: Some(Agent::Pi),
            pending_foreground_shell_clear: true,
            ..process_probe_input()
        }
    ));
    assert!(!should_skip_process_probe_for_lifecycle_authority(
        true,
        ProcessProbeInput {
            current_agent: Some(Agent::Pi),
            suppressed_agent: Some(Agent::Pi),
            ..process_probe_input()
        }
    ));
}

#[test]
fn probe_policy_lifecycle_owner_preserves_initial_and_job_group_change_probes() {
    assert!(!should_skip_process_probe_for_lifecycle_authority(
        true,
        ProcessProbeInput {
            current_agent: None,
            has_process_probe: false,
            ..process_probe_input()
        }
    ));
    assert!(!should_skip_process_probe_for_lifecycle_authority(
        true,
        ProcessProbeInput {
            current_agent: Some(Agent::Pi),
            foreground_pgid: Some(43),
            ..process_probe_input()
        }
    ));
}

#[test]
fn probe_policy_pending_release_forces_initial_scan() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        suppressed_agent: Some(Agent::Codex),
        has_process_probe: false,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_pending_release_forces_scan_after_runtime_identity_clears() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        suppressed_agent: Some(Agent::Codex),
        has_process_probe: false,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_pending_release_skips_repeated_probe_when_job_group_is_stable() {
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        suppressed_agent: Some(Agent::Codex),
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_pending_release_probes_when_job_group_changes() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        suppressed_agent: Some(Agent::Codex),
        foreground_pgid: Some(43),
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_acquisition_window_catches_delayed_same_group_wrapper_startup() {
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        acquisition_age: Some(std::time::Duration::from_millis(1250)),
        elapsed_since_process_check: PROCESS_ACQUISITION_FAST_RECHECK
            .checked_sub(std::time::Duration::from_millis(1))
            .unwrap(),
        ..process_probe_input()
    }));
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        acquisition_age: Some(std::time::Duration::from_millis(1250)),
        elapsed_since_process_check: PROCESS_ACQUISITION_FAST_RECHECK,
        ..process_probe_input()
    }));
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        acquisition_age: Some(std::time::Duration::from_secs(5)),
        elapsed_since_process_check: PROCESS_ACQUISITION_SLOW_RECHECK,
        ..process_probe_input()
    }));
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        acquisition_age: Some(PROCESS_ACQUISITION_WINDOW + std::time::Duration::from_millis(1),),
        elapsed_since_process_check: PROCESS_ACQUISITION_SLOW_RECHECK,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_release_expiry_can_force_reacquire_probe_by_resetting_probe_state() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: None,
        has_process_probe: false,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_identified_agent_uses_shorter_safety_scan() {
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED
            .checked_sub(std::time::Duration::from_millis(1))
            .unwrap(),
        ..process_probe_input()
    }));
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED,
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_identified_agent_probes_when_job_group_disappears() {
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        foreground_pgid: None,
        last_foreground_pgid: Some(42),
        elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED
            .checked_sub(std::time::Duration::from_millis(1))
            .unwrap(),
        ..process_probe_input()
    }));
}

#[test]
fn probe_policy_stable_missing_job_group_uses_safety_scan() {
    assert!(!should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        foreground_pgid: None,
        last_foreground_pgid: None,
        elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED
            .checked_sub(std::time::Duration::from_millis(1))
            .unwrap(),
        ..process_probe_input()
    }));
    assert!(should_probe_foreground_job(ProcessProbeInput {
        current_agent: Some(Agent::Codex),
        foreground_pgid: None,
        last_foreground_pgid: None,
        elapsed_since_process_check: PROCESS_RECHECK_IDENTIFIED,
        ..process_probe_input()
    }));
}

#[test]
fn acquisition_starts_bounded_unknown_acquisition_window() {
    let now = std::time::Instant::now();
    let mut window = AcquisitionWindow::default();

    window.sync(
        ProcessProbeInput {
            current_agent: None,
            suppressed_agent: None,
            foreground_pgid: Some(42),
            ..process_probe_input()
        },
        true,
        now,
    );
    assert_eq!(window.started_at, Some(now));
    assert_eq!(window.last_content_change_at, Some(now));

    let later = now + std::time::Duration::from_secs(1);
    window.sync(
        ProcessProbeInput {
            current_agent: None,
            suppressed_agent: None,
            foreground_pgid: Some(42),
            ..process_probe_input()
        },
        true,
        later,
    );
    assert_eq!(
        window.started_at,
        Some(now),
        "changed frames should not refresh the acquisition window"
    );
    assert_eq!(window.last_content_change_at, Some(later));

    let quiet_after_window = later + PROCESS_ACQUISITION_WINDOW + PROCESS_ACQUISITION_IDLE_RESET;
    window.sync(
        ProcessProbeInput {
            current_agent: None,
            suppressed_agent: None,
            foreground_pgid: Some(42),
            ..process_probe_input()
        },
        false,
        quiet_after_window,
    );
    assert_eq!(window.started_at, None);
    assert_eq!(window.last_content_change_at, None);

    let next_burst = quiet_after_window + std::time::Duration::from_secs(1);
    window.sync(
        ProcessProbeInput {
            current_agent: None,
            suppressed_agent: None,
            foreground_pgid: Some(42),
            ..process_probe_input()
        },
        true,
        next_burst,
    );
    assert_eq!(window.started_at, Some(next_burst));
    assert_eq!(window.last_content_change_at, Some(next_burst));
}

#[test]
fn acquisition_does_not_start_acquisition_when_process_probe_has_other_signal() {
    let now = std::time::Instant::now();
    let mut window = AcquisitionWindow::default();

    window.sync(
        ProcessProbeInput {
            current_agent: Some(Agent::Codex),
            suppressed_agent: None,
            foreground_pgid: Some(42),
            ..process_probe_input()
        },
        true,
        now,
    );
    assert_eq!(window.started_at, None);
    assert_eq!(window.last_content_change_at, None);

    window.sync(
        ProcessProbeInput {
            current_agent: None,
            suppressed_agent: Some(Agent::Codex),
            foreground_pgid: Some(42),
            ..process_probe_input()
        },
        true,
        now,
    );
    assert_eq!(window.started_at, None);
    assert_eq!(window.last_content_change_at, None);

    window.sync(
        ProcessProbeInput {
            current_agent: None,
            suppressed_agent: None,
            foreground_pgid: Some(43),
            ..process_probe_input()
        },
        true,
        now,
    );
    assert_eq!(window.started_at, None);
    assert_eq!(window.last_content_change_at, None);
}

#[test]
fn acquisition_restarts_stale_process_group_acquisition_window() {
    let now = std::time::Instant::now();
    let stale_start = now
        .checked_sub(PROCESS_ACQUISITION_WINDOW + std::time::Duration::from_millis(1))
        .unwrap();
    let mut window = AcquisitionWindow {
        started_at: Some(stale_start),
        last_content_change_at: None,
    };

    window.sync(
        ProcessProbeInput {
            current_agent: None,
            suppressed_agent: None,
            foreground_pgid: Some(42),
            ..process_probe_input()
        },
        true,
        now,
    );

    assert_eq!(window.started_at, Some(now));
    assert_eq!(window.last_content_change_at, Some(now));
}
