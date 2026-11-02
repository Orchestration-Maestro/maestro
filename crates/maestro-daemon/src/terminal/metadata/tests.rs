//! Retained pane behavior tests.

use std::time::Duration;

use super::*;
use crate::terminal::state::tests::test_terminal;
use crate::terminal::state::{DetectedStateUpdate, HookReport};
use maestro_detection::agents::Agent;
use maestro_detection::agents::AgentVisibility;

#[test]
fn pane_presentation_sequences_remain_unbounded_while_token_sequences_are_bounded() {
    let mut terminal = test_terminal();
    for index in 0..=maestro_workspace::metadata_tokens::MAX_SEQUENCE_SOURCES {
        assert_eq!(
            terminal.accept_metadata_report(&format!("source-{index}"), Some(1), false, None,),
            Ok(true)
        );
    }
    for index in 0..maestro_workspace::metadata_tokens::MAX_SEQUENCE_SOURCES {
        assert_eq!(
            terminal.accept_metadata_report(&format!("source-{index}"), Some(2), true, None,),
            Ok(true)
        );
    }
    assert_eq!(
        terminal.accept_metadata_report(
            &format!(
                "source-{}",
                maestro_workspace::metadata_tokens::MAX_SEQUENCE_SOURCES
            ),
            Some(2),
            true,
            None,
        ),
        Err(())
    );
}

#[test]
fn pane_custom_metadata_reanchors_sequence_after_process_restart() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Pi), AgentState::Idle);

    assert!(
        terminal
            .set_agent_metadata(custom_path_metadata_report(100, Some(Duration::ZERO)))
            .is_some()
    );
    let deadline = terminal.next_agent_metadata_expiry().unwrap();
    terminal.expire_agent_metadata_at(deadline, deadline);
    assert!(terminal.agent_metadata.is_empty());
    let exit_at = Instant::now() + Duration::from_millis(1);
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
        exit_at,
    );
    terminal.set_detected_state_with_screen_signals_at(
        DetectedStateUpdate {
            agent: None,
            fallback_state: AgentState::Unknown,
            visible: AgentVisibility {
                visible_blocker: false,
                visible_idle: false,
                visible_working: false,
            },
            process_exited: false,
        },
        exit_at + Duration::from_millis(1),
    );
    assert!(
        terminal
            .set_agent_metadata(custom_path_metadata_report(1, None))
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
        exit_at + Duration::from_millis(2),
    );
    assert!(
        terminal
            .set_agent_metadata(custom_path_metadata_report(1, None))
            .is_some()
    );
}

#[test]
fn pane_user_agent_metadata_overrides_presentation_fields_only() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    let mutation = terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:presentation".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Refactor auth".into()),
        display_agent: Some("Claude: auth".into()),
        state_labels: HashMap::from([("working".into(), "deep in the mines".into())]),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: None,
    });

    assert_eq!(terminal.state, AgentState::Working);
    assert_eq!(terminal.effective_agent_label(), Some("claude"));
    let presentation = terminal.effective_presentation();
    assert_eq!(presentation.title.as_deref(), Some("Refactor auth"));
    assert_eq!(presentation.display_agent.as_deref(), Some("Claude: auth"));
    assert_eq!(
        presentation.state_labels.get("working").map(String::as_str),
        Some("deep in the mines")
    );
    assert!(mutation.unwrap().effective_state_change.is_some());
}

#[test]
fn pane_metadata_title_takes_pane_border_priority() {
    let mut terminal = test_terminal();
    terminal.set_detected_state(Some(Agent::Claude), AgentState::Idle);
    terminal.set_manual_label("manual");
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:presentation".into(),
        agent_label: Some("claude".into()),
        applies_to_source: None,
        title: Some("Prompt title".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: None,
    });

    assert_eq!(
        terminal.border_label(false).as_deref(),
        Some("Prompt title")
    );
    assert_eq!(terminal.border_label(true).as_deref(), Some("Prompt title"));
}

#[test]
fn pane_metadata_without_sequence_can_update_same_source() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    for title in ["first", "second"] {
        terminal.set_agent_metadata(AgentMetadataReport {
            source: "user:claude-title".into(),
            agent_label: Some("claude".into()),
            applies_to_source: Some("maestro:claude".into()),
            title: Some(title.into()),
            display_agent: None,
            state_labels: HashMap::new(),
            clear_title: false,
            clear_display_agent: false,
            clear_state_labels: false,
            ttl: None,
            seq: None,
        });
    }

    assert_eq!(terminal.effective_title().as_deref(), Some("second"));
}

#[test]
fn pane_metadata_resolves_newest_value_per_presentation_field() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:title".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Prompt title".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: Some(1),
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: None,
        display_agent: Some("Claude activity".into()),
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: Some(1),
    });

    let presentation = terminal.effective_presentation();
    assert_eq!(presentation.title.as_deref(), Some("Prompt title"));
    assert_eq!(
        presentation.display_agent.as_deref(),
        Some("Claude activity")
    );
}

#[test]
fn pane_partial_update_does_not_refresh_unchanged_field_precedence() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:first".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: None,
        display_agent: Some("First display".into()),
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: Some(1),
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:first".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Fresh title".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: true,
        clear_state_labels: false,
        ttl: None,
        seq: Some(2),
    });

    let presentation = terminal.effective_presentation();
    assert_eq!(presentation.title.as_deref(), Some("Fresh title"));
    assert_eq!(presentation.display_agent, None);
}

fn prompt_metadata_report() -> AgentMetadataReport {
    AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: None,
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: None,
    }
}

#[test]
fn pane_metadata_clear_plus_set_without_ttl_does_not_keep_old_ttl() {
    let mut terminal = test_terminal();
    let now = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.replace_agent_metadata(
        AgentMetadataReport {
            title: Some("Old title".into()),
            ttl: Some(Duration::from_millis(1)),
            ..prompt_metadata_report()
        },
        now,
    );
    assert_eq!(terminal.agent_metadata["user:status"].reported_at, now);
    let old_deadline =
        TerminalState::agent_metadata_expiry(&terminal.agent_metadata["user:status"]).unwrap();

    terminal.update_agent_metadata_fields(
        AgentMetadataReport {
            title: Some("Fresh title".into()),
            clear_title: true,
            ttl: None,
            ..prompt_metadata_report()
        },
        now + Duration::from_micros(500),
    );

    assert_eq!(
        TerminalState::agent_metadata_expiry(&terminal.agent_metadata["user:status"]),
        None
    );
    assert_eq!(
        terminal
            .effective_presentation_for_state_at(terminal.state, now + Duration::from_micros(500))
            .title
            .as_deref(),
        Some("Fresh title")
    );
    assert!(
        terminal
            .expire_agent_metadata_at(
                old_deadline + Duration::from_millis(1),
                old_deadline + Duration::from_millis(1)
            )
            .is_none()
    );
    assert_eq!(
        terminal
            .effective_presentation_for_state_at(terminal.state, now + Duration::from_micros(500))
            .title
            .as_deref(),
        Some("Fresh title")
    );
}

#[test]
fn pane_metadata_clear_only_without_ttl_does_not_extend_old_ttl() {
    let mut terminal = test_terminal();
    let now = Instant::now().checked_sub(Duration::from_secs(1)).unwrap();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.replace_agent_metadata(
        AgentMetadataReport {
            title: Some("Prompt title".into()),
            display_agent: Some("Old display".into()),
            ttl: Some(Duration::from_millis(1)),
            ..prompt_metadata_report()
        },
        now,
    );
    assert_eq!(terminal.agent_metadata["user:status"].reported_at, now);
    let old_deadline =
        TerminalState::agent_metadata_expiry(&terminal.agent_metadata["user:status"]).unwrap();

    terminal.update_agent_metadata_fields(
        AgentMetadataReport {
            source: "user:status".into(),
            agent_label: None,
            applies_to_source: None,
            title: None,
            display_agent: None,
            state_labels: HashMap::new(),
            clear_title: false,
            clear_display_agent: true,
            clear_state_labels: false,
            ttl: None,
            seq: None,
        },
        now + Duration::from_micros(500),
    );

    assert_eq!(
        TerminalState::agent_metadata_expiry(&terminal.agent_metadata["user:status"]),
        Some(old_deadline)
    );
    let presentation = terminal
        .effective_presentation_for_state_at(terminal.state, now + Duration::from_micros(500));
    assert_eq!(presentation.title.as_deref(), Some("Prompt title"));
    assert_eq!(presentation.display_agent, None);

    let mutation = terminal
        .expire_agent_metadata_at(old_deadline, old_deadline)
        .unwrap();
    assert!(mutation.effective_state_change.is_some());
    assert_eq!(
        terminal
            .effective_presentation_for_state_at(terminal.state, old_deadline)
            .title,
        None
    );
}

#[test]
fn pane_metadata_ttl_expiry_reports_presentation_change() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Activity".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: Some(Duration::from_millis(1)),
        seq: None,
    });

    let deadline = terminal.next_agent_metadata_expiry().unwrap();
    let mutation = terminal
        .expire_agent_metadata_at(deadline, deadline)
        .unwrap();
    let change = mutation.effective_state_change.unwrap();

    assert_eq!(change.previous_state, AgentState::Working);
    assert_eq!(change.state, AgentState::Working);
    assert_eq!(
        change.previous_presentation.title.as_deref(),
        Some("Activity")
    );
    assert_eq!(change.presentation.title, None);
    assert_eq!(terminal.effective_title(), None);
}

#[test]
fn pane_stale_guarded_metadata_expiry_does_not_report_visible_change() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Stale".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: Some(Duration::from_millis(1)),
        seq: None,
    });
    let deadline = terminal
        .agent_metadata
        .get("user:status")
        .and_then(TerminalState::agent_metadata_expiry)
        .unwrap();
    assert_eq!(terminal.next_agent_metadata_expiry(), None);
    assert_eq!(terminal.effective_title(), None);

    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    let mutation = terminal.expire_agent_metadata_at(
        deadline + Duration::from_millis(1),
        deadline + Duration::from_millis(1),
    );

    assert!(mutation.is_none());
    assert!(!terminal.agent_metadata.contains_key("user:status"));
    assert_eq!(terminal.effective_title(), None);
}

#[test]
fn pane_late_metadata_expiry_reports_all_due_visible_changes() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:first".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("First".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: Some(Duration::from_millis(1)),
        seq: None,
    });
    let first_deadline = terminal.next_agent_metadata_expiry().unwrap();
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:second".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: None,
        display_agent: Some("Second".into()),
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: Some(Duration::from_millis(2)),
        seq: None,
    });
    let second_deadline = terminal
        .agent_metadata
        .get("user:second")
        .and_then(TerminalState::agent_metadata_expiry)
        .unwrap();

    let mutation = terminal
        .expire_agent_metadata_at(first_deadline, second_deadline)
        .unwrap();
    let change = mutation.effective_state_change.unwrap();

    assert_eq!(change.previous_presentation.title.as_deref(), Some("First"));
    assert_eq!(
        change.previous_presentation.display_agent.as_deref(),
        Some("Second")
    );
    assert_eq!(change.presentation, EffectivePresentation::empty());
    assert!(terminal.agent_metadata.is_empty());
}

#[test]
fn pane_immediately_expired_visible_metadata_still_schedules_expiry_event() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Instant".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: Some(Duration::ZERO),
        seq: None,
    });

    let deadline = terminal
        .next_agent_metadata_expiry()
        .expect("pending expiry should be scheduled");
    let mutation = terminal
        .expire_agent_metadata_at(deadline, Instant::now())
        .unwrap();
    let change = mutation.effective_state_change.unwrap();

    assert_eq!(
        change.previous_presentation.title.as_deref(),
        Some("Instant")
    );
    assert_eq!(change.presentation.title, None);
    assert_eq!(terminal.effective_title(), None);
}

#[test]
fn pane_pending_metadata_expiry_clears_when_lifecycle_guard_hides_metadata() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Instant".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: Some(Duration::ZERO),
        seq: None,
    });
    assert!(terminal.next_agent_metadata_expiry().is_some());

    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    assert_eq!(terminal.next_agent_metadata_expiry(), None);
    assert_eq!(terminal.effective_title(), None);
}

#[test]
fn pane_partial_update_does_not_resurrect_expired_hidden_metadata_fields() {
    let mut terminal = test_terminal();
    terminal.set_hook_authority(HookReport {
        source: "maestro:codex".into(),
        agent_label: "codex".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });
    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: Some("Expired title".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: Some(Duration::ZERO),
        seq: None,
    });
    assert_eq!(terminal.next_agent_metadata_expiry(), None);

    terminal.set_agent_metadata(AgentMetadataReport {
        source: "user:status".into(),
        agent_label: Some("claude".into()),
        applies_to_source: Some("maestro:claude".into()),
        title: None,
        display_agent: Some("Fresh display".into()),
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl: None,
        seq: None,
    });
    terminal.set_hook_authority(HookReport {
        source: "maestro:claude".into(),
        agent_label: "claude".into(),
        state: AgentState::Working,
        message: None,
        session_ref: None,
        seq: None,
    });

    let presentation = terminal.effective_presentation();
    assert_eq!(presentation.title, None);
    assert_eq!(presentation.display_agent.as_deref(), Some("Fresh display"));
}

mod aliases;

fn custom_path_metadata_report(seq: u64, ttl: Option<Duration>) -> AgentMetadataReport {
    AgentMetadataReport {
        source: "custom:pi-metadata".into(),
        agent_label: Some("pi".into()),
        applies_to_source: None,
        title: Some("Pi task".into()),
        display_agent: None,
        state_labels: HashMap::new(),
        clear_title: false,
        clear_display_agent: false,
        clear_state_labels: false,
        ttl,
        seq: Some(seq),
    }
}
