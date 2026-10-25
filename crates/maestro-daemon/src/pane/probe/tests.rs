use super::*;
fn foreground_process(pid: u32, name: &str) -> maestro_process::pty::foreground::ForegroundProcess {
    maestro_process::pty::foreground::ForegroundProcess {
        pid,
        name: name.to_string(),
        argv0: None,
        argv: None,
        cmdline: None,
    }
}

#[test]
fn job_hint_accepts_pane_shell_environment() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 42,
        processes: vec![foreground_process(42, "bash")],
    };

    assert_eq!(
        agent_hint_for_foreground_job_members(&job, |pid| { (pid == 42).then_some(Agent::Claude) }),
        Some(Agent::Claude)
    );
}

#[test]
fn job_hint_accepts_non_leader_foreground_process_environment() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 99,
        processes: vec![
            foreground_process(99, "fence"),
            foreground_process(100, "pi"),
        ],
    };

    assert_eq!(
        agent_hint_for_foreground_job_members(&job, |pid| { (pid == 100).then_some(Agent::Codex) }),
        Some(Agent::Codex)
    );
}

#[test]
fn job_hint_wins_over_process_name_detection() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 99,
        processes: vec![foreground_process(99, "codex")],
    };

    let result = probe_foreground_process_from_jobs(
        42,
        Some(99),
        Some(&job),
        || None,
        |pid| (pid == 99).then_some(Agent::Claude),
    );

    assert_eq!(result.agent, Some(Agent::Claude));
    assert_eq!(result.process_name.as_deref(), Some("claude"));
}

#[test]
fn job_hint_on_inherited_child_environment_is_authoritative() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 99,
        processes: vec![foreground_process(99, "vim")],
    };

    let result = probe_foreground_process_from_jobs(
        42,
        Some(99),
        None,
        || Some(job),
        |pid| (pid == 99).then_some(Agent::Claude),
    );

    assert_eq!(result.agent, Some(Agent::Claude));
    assert_eq!(result.process_name.as_deref(), Some("claude"));
}

#[test]
fn job_hint_member_hint_does_not_override_identifiable_leader() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 99,
        processes: vec![
            foreground_process(99, "codex"),
            foreground_process(100, "vim"),
        ],
    };

    let result = probe_foreground_process_from_jobs(
        42,
        Some(99),
        None,
        || Some(job),
        |pid| (pid == 100).then_some(Agent::Claude),
    );

    assert_eq!(result.agent, Some(Agent::Codex));
    assert_eq!(result.process_name.as_deref(), Some("codex"));
}

#[test]
fn job_hint_member_hint_wins_when_leader_is_unidentified() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 99,
        processes: vec![
            foreground_process(99, "some_vm"),
            foreground_process(100, "vim"),
        ],
    };

    let result = probe_foreground_process_from_jobs(
        42,
        Some(99),
        None,
        || Some(job),
        |pid| (pid == 100).then_some(Agent::Claude),
    );

    assert_eq!(result.agent, Some(Agent::Claude));
    assert_eq!(result.process_name.as_deref(), Some("claude"));
}

#[test]
fn environment_hint_recognizes_catalog_labels() {
    assert_eq!(parse_agent_env_hint(Some("claude")), Some(Agent::Claude));
    assert_eq!(parse_agent_env_hint(Some("codex")), Some(Agent::Codex));
}

#[test]
fn environment_hint_ignores_absent_or_unknown_values() {
    assert_eq!(parse_agent_env_hint(None), None);
    assert_eq!(parse_agent_env_hint(Some("not-an-agent")), None);
}

#[test]
fn group_name_fallback_preserves_shell_membership_and_unknown_jobs() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 42,
        processes: vec![
            foreground_process(42, "bash"),
            foreground_process(43, "codex"),
        ],
    };
    let result = probe_foreground_process_from_jobs(42, Some(42), None, || Some(job), |_| None);
    assert_eq!(result.agent, Some(Agent::Codex));
    assert_eq!(result.process_name.as_deref(), Some("codex"));
    assert!(result.foreground_is_pane_shell);

    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 42,
        processes: vec![foreground_process(42, "bash")],
    };
    let result = probe_foreground_process_from_jobs(42, Some(42), None, || Some(job), |_| None);
    assert_eq!(result.agent, None);
    assert_eq!(result.process_name, None);
    assert!(result.foreground_is_pane_shell);
    assert_eq!(result.process_group_id, Some(42));
}

#[test]
fn leader_identity_avoids_unnecessary_full_job_scan() {
    let job = maestro_process::pty::foreground::ForegroundJob {
        process_group_id: 42,
        processes: vec![foreground_process(42, "codex")],
    };
    let result = probe_foreground_process_from_jobs(
        41,
        Some(42),
        Some(&job),
        || panic!("leader already identifies the job"),
        |_| None,
    );
    assert_eq!(result.agent, Some(Agent::Codex));
    assert!(!result.foreground_is_pane_shell);
}

#[test]
fn absent_native_job_preserves_only_the_observed_group() {
    let result = probe_foreground_process(0, None, ProcessDetectionMode::Native);
    assert_eq!(
        result,
        ProcessProbeResult {
            process_group_id: None,
            foreground_is_pane_shell: false,
            agent: None,
            process_name: None,
        }
    );
    let result = probe_foreground_process_from_jobs(0, Some(42), None, || None, |_| None);
    assert_eq!(result.process_group_id, Some(42));
    assert_eq!(result.agent, None);
}
