//! Canonical metadata map, sequence and guard admission.
use super::*;
use crate::terminal::model::HookReport;

#[test]
fn metadata_source_aliases_share_sequences_and_guards() {
    for (source, alias) in [
        ("herdr:claude", "maestro:claude"),
        ("maestro:claude", "herdr:claude"),
    ] {
        let mut terminal = test_terminal();
        terminal.set_detected_state(Some(Agent::Claude), AgentState::Idle);

        terminal
            .set_hook_authority_with_session_ref(HookReport {
                source: alias.into(),
                agent_label: "claude".into(),
                state: AgentState::Working,
                message: None,
                session_ref: None,
                seq: Some(1),
            })
            .unwrap();
        terminal
            .set_agent_metadata(report(source, alias, 7, "first"))
            .unwrap();
        assert_eq!(terminal.effective_title().as_deref(), Some("first"));
        let metadata = terminal.agent_metadata.get("maestro:claude").unwrap();
        assert_eq!(metadata.source, "maestro:claude");
        assert_eq!(
            metadata.applies_to_source.as_deref(),
            Some("maestro:claude")
        );
        assert!(
            terminal
                .set_agent_metadata(report(alias, source, 7, "stale"))
                .is_none()
        );
        assert_eq!(terminal.effective_title().as_deref(), Some("first"));
        terminal
            .set_agent_metadata(report(alias, source, 8, "fresh"))
            .unwrap();
        assert_eq!(terminal.effective_title().as_deref(), Some("fresh"));
        assert_eq!(terminal.agent_metadata.len(), 1);
        assert_eq!(terminal.metadata_report_sequences.len(), 1);
        assert_eq!(
            terminal.metadata_report_sequences.get("maestro:claude"),
            Some(&8)
        );
        terminal
            .clear_hook_authority_with_mutation(Some(source), Some(2))
            .unwrap();
        assert_eq!(terminal.effective_title(), None);
        assert!(
            terminal
                .set_hook_authority_with_session_ref(HookReport {
                    source: alias.into(),
                    agent_label: "claude".into(),
                    state: AgentState::Working,
                    message: None,
                    session_ref: None,
                    seq: Some(3),
                })
                .is_some()
        );
    }
}

fn report(source: &str, guard: &str, seq: u64, title: &str) -> AgentMetadataReport {
    AgentMetadataReport {
        source: source.into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some(guard.into()),
        title: Some(title.into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: Some(seq),
    }
}
