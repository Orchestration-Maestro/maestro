//! Construction and retained-source boundary behavior.

use super::*;
use crate::resume::{
    self as agent_resume, AgentResumePlan, AgentSessionRef, PersistedAgentSession,
};

#[test]
fn terminal_title_updates_distinguish_raw_and_presented_changes() {
    let mut terminal = test_terminal();
    assert_eq!(terminal.terminal_title_stripped(), None);
    let change = terminal.set_terminal_title(Some("✢ Build".into()));
    assert!(change.raw_changed && change.stripped_changed);
    assert_eq!(terminal.revision, 1);
    let change = terminal.set_terminal_title(Some("✳ Build".into()));
    assert!(change.raw_changed && !change.stripped_changed);
    assert_eq!(terminal.terminal_title_stripped().as_deref(), Some("Build"));
    assert_eq!(terminal.revision, 1);
    assert_eq!(
        terminal.set_terminal_title(Some("✳ Build".into())),
        TerminalTitleChange::default()
    );
    terminal.revision = u64::MAX;
    let change = terminal.set_terminal_title(Some("Review".into()));
    assert!(change.raw_changed && change.stripped_changed);
    assert_eq!(terminal.revision, 0);
    let change = terminal.set_terminal_title(None);
    assert!(change.raw_changed && change.stripped_changed);
    assert_eq!(terminal.terminal_title_stripped(), None);
    assert_eq!(terminal.revision, 1);
}

#[test]
fn agent_acquisition_finishes_only_at_idle_without_recent_exit() {
    let mut terminal = test_terminal();
    let now = Instant::now();
    terminal.set_detected_agent_process_at(Agent::Pi, now);
    assert!(!terminal.finish_agent_process_acquisition());
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Working);
    assert!(!terminal.finish_agent_process_acquisition());
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    assert!(terminal.finish_agent_process_acquisition());
    assert!(!terminal.finish_agent_process_acquisition());
    let exit_at = now + Duration::from_secs(1);
    terminal.set_detected_agent_process_at(Agent::Pi, exit_at);
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: Some(Agent::Pi),
            fallback_state: AgentState::Idle,
            visible: AgentVisibility {
                visible_idle: false,
                visible_blocker: false,
                visible_working: false,
            },
            process_exited: true,
        },
        exit_at,
    );
    assert!(!terminal.finish_agent_process_acquisition());
    assert!(!terminal.finish_agent_process_acquisition());
    assert!(terminal.agent_process_exited_within(exit_at, Duration::ZERO));
    assert!(
        !terminal.agent_process_exited_within(exit_at + Duration::from_secs(1), Duration::ZERO)
    );
}

#[test]
fn terminal_construction_retains_launch_and_resume_values() {
    let terminal = test_terminal();
    assert_terminal_defaults(&terminal);
    let plan = AgentResumePlan {
        agent: "pi".into(),
        argv: vec!["pi".into(), "--session".into(), "literal ; $x".into()],
        dedupe_key: "retained".into(),
    };
    let argv = vec!["pi".into(), "--flag with space".into()];
    let terminal = terminal
        .with_launch_argv(argv.clone())
        .with_respawn_shell_on_exit()
        .with_pending_agent_resume_plan(plan.clone());
    assert_eq!(terminal.cwd, PathBuf::from("/tmp"));
    assert_eq!(terminal.launch_argv, Some(argv));
    assert!(terminal.respawn_shell_on_exit);
    assert_eq!(terminal.pending_agent_resume_plan, Some(plan));
    assert!(!terminal.is_agent_terminal());
    assert_eq!(terminal.effective_agent_label(), None);
}

fn assert_terminal_defaults(terminal: &TerminalState) {
    assert_eq!(terminal.cwd, PathBuf::from("/tmp"));
    assert_eq!(terminal.state, AgentState::Unknown);
    assert_eq!(terminal.fallback_state, AgentState::Unknown);
    assert!(terminal.detected_agent.is_none() && terminal.hook_authority.is_none());
    assert!(terminal.agent_metadata.is_empty() && terminal.metadata_tokens.values().is_empty());
    assert!(terminal.persisted_agent_session.is_none() && terminal.terminal_title.is_none());
    assert!(terminal.manual_label.is_none() && terminal.agent_name.is_none());
    assert_eq!(terminal.last_agent_state_change_seq, None);
    assert_eq!(terminal.revision, 0);
    assert_eq!(terminal.launch_argv, None);
    assert!(!terminal.respawn_shell_on_exit);
    assert_eq!(terminal.pending_agent_resume_plan, None);
}

#[test]
fn label_selection_keeps_metadata_manual_and_agent_priorities() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
    assert_eq!(terminal.border_label(true).as_deref(), Some("pi"));
    assert_eq!(terminal.border_label(false), None);
    terminal.set_agent_name("  reviewer  ".into());
    assert_eq!(terminal.agent_name.as_deref(), Some("  reviewer  "));
    // Names retain session ownership; border labels still use presentation identity.
    assert_eq!(terminal.border_label(true).as_deref(), Some("pi"));
    terminal.set_manual_label("  manual  ");
    assert_eq!(terminal.border_label(false).as_deref(), Some("manual"));
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:presentation".into(),
        agent_label: None,
        applies_to_source: None,
        title: Some("Metadata".into()),
        display_agent: Some("Displayed".into()),
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: None,
    });
    assert_eq!(terminal.border_label(false).as_deref(), Some("Metadata"));
    terminal.agent_metadata.clear();
    assert_eq!(terminal.border_label(true).as_deref(), Some("manual"));
    terminal.set_manual_label(" \t ");
    assert_eq!(terminal.manual_label, None);
    assert_eq!(terminal.border_label(true).as_deref(), Some("pi"));
    terminal.set_manual_label("temporary");
    terminal.clear_manual_label();
    terminal.set_agent_name(String::new());
    assert_eq!(terminal.agent_name, None);
    assert_eq!(terminal.border_label(false), None);
}

#[test]
fn retained_hook_and_resume_sources_have_one_canonical_identity() {
    for (source, alias) in [("herdr:pi", "maestro:pi"), ("maestro:pi", "herdr:pi")] {
        let mut terminal = test_terminal();
        terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);
        terminal.set_persisted_agent_session(PersistedAgentSession {
            source: source.into(),
            agent: "pi".into(),
            session_ref: AgentSessionRef::id("old").unwrap(),
        });
        assert_eq!(
            terminal.persisted_agent_session.as_ref().unwrap().source,
            "maestro:pi"
        );
        let session_ref = AgentSessionRef::id("new").unwrap();
        terminal
            .set_agent_session_ref_for_session_start(SessionStartReport {
                source: source.into(),
                agent_label: "pi".into(),
                session_ref: Some(session_ref.clone()),
                seq: Some(1),
                session_start_source: Some("new".into()),
            })
            .unwrap();
        assert_eq!(
            terminal.persisted_agent_session.as_ref().unwrap().source,
            "maestro:pi"
        );
        let report = HookReport {
            source: source.into(),
            agent_label: "pi".into(),
            state: AgentState::Working,
            message: None,
            session_ref: Some(session_ref.clone()),
            seq: Some(2),
        };
        terminal
            .set_hook_authority_with_session_ref(report.clone())
            .unwrap();
        assert_canonical_hook_clear_and_release(
            &mut terminal,
            report,
            session_ref,
            (source, alias),
        );
    }
}

fn assert_canonical_hook_clear_and_release(
    terminal: &mut TerminalState,
    report: HookReport,
    session_ref: AgentSessionRef,
    sources: (&str, &str),
) {
    let (source, alias) = sources;
    let authority = terminal.hook_authority.as_ref().unwrap();
    assert_eq!(authority.source, "maestro:pi");
    let plan = agent_resume::plan(&authority.source, "pi", &session_ref).unwrap();
    assert_eq!(plan.argv, ["pi", "--session", "new"]);
    assert_eq!(plan.dedupe_key, "maestro:pi\0pi\0Id\0new");
    assert!(
        terminal
            .set_hook_authority_with_session_ref(HookReport {
                source: alias.into(),
                state: AgentState::Idle,
                ..report
            })
            .is_none()
    );
    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(terminal.hook_report_sequences.len(), 1);
    assert_eq!(terminal.hook_report_sequences.get("maestro:pi"), Some(&2));
    terminal
        .clear_hook_authority_with_mutation(Some(alias), Some(3))
        .unwrap();
    assert!(terminal.hook_authority.is_none());
    assert!(
        terminal
            .suppressed_full_lifecycle_hook_reports
            .contains_key("maestro:pi")
    );
    terminal.set_persisted_agent_session(PersistedAgentSession {
        source: source.into(),
        agent: "pi".into(),
        session_ref,
    });
    terminal
        .release_agent_with_mutation(alias, "pi", Some(4))
        .unwrap();
    assert!(terminal.persisted_agent_session.is_none());
    assert_eq!(terminal.hook_report_sequences.len(), 1);
}
