//! Pane metadata reports through the daemon's handler: validation, tokens, presentation,
//! sequences and publication.
#![cfg(target_os = "linux")]
use crate::support::api::{
    TestApp, daemon_with_workspaces, detect, events_after, pilot, request, root_pane,
};
use maestro_daemon::terminal::state::{DetectedStateUpdate, TerminalState};
use maestro_detection::agents::{Agent, AgentState, AgentVisibility};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, PaneInfo, ResponseResult},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

/// The longest lifetime a report may ask for, in milliseconds.
const MAX_TTL_MS: u64 = 86_400_000;

/// A report for the first pane from `source` that carries exactly `fields`.
fn report(source: &str, fields: Value) -> Value {
    let mut wire = json!({"type": "pane.report_metadata", "pane_id": "w1:p1", "source": source});
    let Value::Object(fields) = fields else {
        panic!("report fields are an object")
    };
    wire.as_object_mut().unwrap().extend(fields);
    wire
}

/// A title report from `user:metadata.test-1` for the first pane, with `fields` merged over it.
fn titled(fields: Value) -> Value {
    let mut wire = report("user:metadata.test-1", json!({"title": "activity"}));
    let Value::Object(fields) = fields else {
        panic!("report fields are an object")
    };
    wire.as_object_mut().unwrap().extend(fields);
    wire
}

/// Answers `wire`, which must succeed with the plain acknowledgement.
fn accepted(app: &mut TestApp, wire: Value) {
    let text = wire.to_string();
    assert_eq!(request(app, wire), Ok(ResponseResult::Ok {}), "{text}");
}

/// The record of the first pane.
fn pane(app: &mut TestApp) -> PaneInfo {
    match request(app, json!({"type": "pane.get", "pane_id": "w1:p1"})) {
        Ok(ResponseResult::PaneInfo { pane }) => pane,
        other => panic!("expected a pane: {other:?}"),
    }
}

/// The tokens the first pane holds.
fn tokens(app: &mut TestApp) -> HashMap<String, String> {
    pane(app).tokens
}

/// What the failure of `wire` is, by code and message.
fn refusal(app: &mut TestApp, wire: Value) -> (String, String) {
    let text = wire.to_string();
    match request(app, wire) {
        Err(ErrorBody { code, message }) => (code, message),
        other => panic!("{text} should be refused: {other:?}"),
    }
}

/// The retained state of the first pane's terminal.
fn terminal_of(app: &TestApp) -> &TerminalState {
    let id = app.state.topology.workspaces[0]
        .terminal_id(root_pane(app, 0))
        .unwrap();
    &app.state.terminals[id]
}

/// The detection of `agent` in `state`, reporting a process exit when `process_exited`.
fn update(agent: Option<Agent>, state: AgentState, process_exited: bool) -> DetectedStateUpdate {
    DetectedStateUpdate {
        agent,
        fallback_state: state,
        visible: AgentVisibility::default(),
        process_exited,
    }
}

/// Feeds the first pane's terminal a detection of `update` at `at`.
fn observe(app: &mut TestApp, update: DetectedStateUpdate, at: Instant) {
    let terminal = app.state.topology.workspaces[0]
        .terminal_id(root_pane(app, 0))
        .unwrap()
        .clone();
    app.state
        .terminals
        .get_mut(&terminal)
        .unwrap()
        .set_detected_state_with_screen_signals_at(update, at);
}

#[test]
fn tokens_patch_and_clear_without_touching_the_presentation() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let set = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "reviewing auth", "model": "opus"}}),
    );
    accepted(&mut app, set);
    let expected = |pairs: &[(&str, &str)]| {
        pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect::<HashMap<_, _>>()
    };
    assert_eq!(
        tokens(&mut app),
        expected(&[("summary", "reviewing auth"), ("model", "opus")])
    );
    let patch = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "done", "model": null}}),
    );
    accepted(&mut app, patch);
    assert_eq!(tokens(&mut app), expected(&[("summary", "done")]));
    assert!(terminal_of(&app).agent_metadata.is_empty());
}

#[test]
fn tokens_ignore_the_agent_guard_of_the_presentation() {
    let mut app = daemon_with_workspaces(&["/one"]);
    detect(&mut app, Agent::Claude, AgentState::Working);
    let wire = report(
        "user:metadata.test-1",
        json!({"agent": "codex", "tokens": {"summary": "global"}}),
    );
    accepted(&mut app, wire);
    assert_eq!(tokens(&mut app)["summary"], "global");
    assert_eq!(pane(&mut app).title, None);
}

#[test]
fn presentation_and_tokens_share_one_sequence_per_source() {
    let mut app = daemon_with_workspaces(&["/one"]);
    accepted(&mut app, titled(json!({"seq": 10})));
    let stale = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "stale"}, "seq": 9}),
    );
    accepted(&mut app, stale);
    assert!(tokens(&mut app).is_empty());
    let fresh = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "fresh"}, "seq": 11}),
    );
    accepted(&mut app, fresh);
    assert_eq!(tokens(&mut app)["summary"], "fresh");
}

#[test]
fn a_report_ignored_after_the_process_exit_does_not_advance_the_sequence() {
    let mut app = daemon_with_workspaces(&["/one"]);
    detect(&mut app, Agent::Pi, AgentState::Idle);
    let initial = titled(json!({"source": "custom:pi-metadata", "agent": "pi", "seq": 100}));
    accepted(&mut app, initial);
    let initial_tokens = report(
        "custom:pi-tokens",
        json!({"agent": "pi", "tokens": {"generation": "old"}, "seq": 100}),
    );
    accepted(&mut app, initial_tokens);
    let exit_at = Instant::now() + Duration::from_millis(1);
    observe(
        &mut app,
        update(Some(Agent::Pi), AgentState::Idle, true),
        exit_at,
    );
    let gone = update(None, AgentState::Unknown, false);
    observe(&mut app, gone, exit_at + Duration::from_millis(1));
    let stale = titled(json!({
        "source": "custom:pi-metadata", "agent": "pi", "title": "stale", "seq": 200
    }));
    accepted(&mut app, stale);
    accepted(
        &mut app,
        titled(json!({"source": "maestro:pi", "seq": 200})),
    );
    let back = update(Some(Agent::Pi), AgentState::Idle, false);
    observe(&mut app, back, exit_at + Duration::from_millis(2));
    let fresh = titled(json!({
        "source": "custom:pi-metadata", "agent": "pi", "title": "fresh", "seq": 1
    }));
    accepted(&mut app, fresh);
    let fresh_tokens = report(
        "custom:pi-tokens",
        json!({"agent": "pi", "tokens": {"generation": "new"}, "seq": 1}),
    );
    accepted(&mut app, fresh_tokens);
    let retained = terminal_of(&app);
    let title = retained.agent_metadata["custom:pi-metadata"]
        .title
        .as_deref();
    assert_eq!(title, Some("fresh"));
    let generation = retained.metadata_tokens.values().get("generation").cloned();
    assert_eq!(generation.as_deref(), Some("new"));
}

#[test]
fn presentation_fields_reach_the_pane_record_beside_the_tokens() {
    let mut app = daemon_with_workspaces(&["/one"]);
    pilot(&mut app, AgentState::Idle);
    let session = json!({
        "type": "pane.report_agent_session", "pane_id": "w1:p1", "source": "maestro:pi",
        "agent": "pi", "agent_session_path": "/sessions/pi.jsonl",
        "session_start_source": "startup", "seq": 1
    });
    accepted(&mut app, session);
    let hook = json!({
        "type": "pane.report_agent", "pane_id": "w1:p1", "source": "maestro:pi", "agent": "pi",
        "state": "working", "agent_session_path": "/sessions/pi.jsonl", "seq": 2
    });
    accepted(&mut app, hook);
    let wire = json!({
        "type": "pane.report_metadata", "pane_id": "w1:p1", "source": "user:pi-display",
        "agent": "pi", "applies_to_source": "maestro:pi", "title": "Refactor auth",
        "display_agent": "Pi auth", "state_labels": {"working": "deep in the mines"},
        "tokens": {"summary": "reviewing auth", "model": "opus"}
    });
    accepted(&mut app, wire);
    let reported = pane(&mut app);
    assert_eq!(reported.agent.as_deref(), Some("pi"));
    assert_eq!(reported.title.as_deref(), Some("Refactor auth"));
    assert_eq!(reported.display_agent.as_deref(), Some("Pi auth"));
    assert_eq!(reported.state_labels["working"], "deep in the mines");
    assert_eq!(reported.tokens["summary"], "reviewing auth");
    assert_eq!(reported.tokens["model"], "opus");
}

#[test]
fn text_is_cleaned_and_state_labels_are_matched_without_case() {
    let mut app = daemon_with_workspaces(&["/one"]);
    detect(&mut app, Agent::Pi, AgentState::Idle);
    let long = format!("  tab\there {}  ", "x".repeat(100));
    let wire = titled(json!({
        "agent": "pi", "title": long, "state_labels": {" Working ": "  busy\u{7}bee "}
    }));
    accepted(&mut app, wire);
    let reported = pane(&mut app);
    let title = reported.title.unwrap();
    assert!(
        title.starts_with("tabhere xxxx") && title.chars().count() == 80,
        "{title:?}"
    );
    assert_eq!(reported.state_labels["working"], "busybee");
}

#[test]
fn a_documented_source_and_the_longest_lifetime_are_accepted() {
    let mut app = daemon_with_workspaces(&["/one"]);
    accepted(&mut app, titled(json!({"ttl_ms": MAX_TTL_MS})));
}

#[test]
fn a_malformed_source_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let long = "a".repeat(81);
    for source in [
        "",
        "   ",
        "user metadata",
        "user/metadata",
        "user:\u{7f}metadata",
        &long,
    ] {
        let (code, _) = refusal(&mut app, titled(json!({"source": source})));
        assert_eq!(code, "invalid_metadata_source", "{source:?}");
    }
    for guard in ["herdr source", "   "] {
        let wire = titled(json!({"applies_to_source": guard}));
        assert_eq!(
            refusal(&mut app, wire).0,
            "invalid_metadata_source",
            "{guard:?}"
        );
    }
}

#[test]
fn a_lifetime_outside_the_supported_range_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for ttl in [0, MAX_TTL_MS + 1] {
        let (code, _) = refusal(&mut app, titled(json!({"ttl_ms": ttl})));
        assert_eq!(code, "invalid_metadata_ttl", "{ttl}");
    }
}

#[test]
fn contradictory_or_empty_requests_are_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let conflict = "cannot set and clear the same metadata field";
    for wire in [
        titled(json!({"clear_title": true})),
        titled(json!({"title": "   ", "clear_title": true})),
        titled(json!({"title": null, "display_agent": "x", "clear_display_agent": true})),
        titled(json!({"title": null, "state_labels": {"idle": "x"}, "clear_state_labels": true})),
    ] {
        let expected = ("invalid_metadata_request".to_owned(), conflict.to_owned());
        assert_eq!(refusal(&mut app, wire.clone()), expected, "{wire}");
    }
    let empty = report("user:metadata.test-1", json!({}));
    let expected = (
        "invalid_metadata_request".to_owned(),
        "missing metadata field to set or clear".to_owned(),
    );
    assert_eq!(refusal(&mut app, empty), expected);
}

#[test]
fn an_unknown_state_label_and_a_malformed_token_are_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let sleepy = titled(json!({"state_labels": {"sleepy": "zzz"}}));
    assert_eq!(
        refusal(&mut app, sleepy),
        (
            "invalid_state_label".to_owned(),
            "unknown state label: sleepy".to_owned()
        )
    );
    let dotted = titled(json!({"tokens": {"bad.name": "v"}}));
    assert_eq!(
        refusal(&mut app, dotted),
        (
            "invalid_metadata_token".to_owned(),
            "invalid metadata token key: bad.name".to_owned()
        )
    );
    let many: HashMap<String, Value> = (0..17).map(|n| (format!("key{n}"), json!("v"))).collect();
    let (code, _) = refusal(&mut app, titled(json!({"tokens": many})));
    assert_eq!(code, "invalid_metadata_token");
}

#[test]
fn a_pane_holds_at_most_thirty_two_tokens_and_a_source_list_is_bounded() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for batch in 0..2 {
        let tokens: HashMap<String, Value> = (0..16)
            .map(|n| (format!("key{batch}x{n}"), json!("v")))
            .collect();
        accepted(&mut app, report("user:fill", json!({"tokens": tokens})));
    }
    let one_more = report("user:fill", json!({"tokens": {"overflow": "v"}}));
    assert_eq!(
        refusal(&mut app, one_more),
        (
            "metadata_token_limit".to_owned(),
            "pane metadata may contain at most 32 tokens".to_owned()
        )
    );
    assert_eq!(tokens(&mut app).len(), 32);
    let mut other = daemon_with_workspaces(&["/two"]);
    for source in 0..32 {
        let wire = report(
            &format!("user:s{source}"),
            json!({"tokens": {"k": "v"}, "seq": 1}),
        );
        accepted(&mut other, wire);
    }
    let wire = report("user:s32", json!({"tokens": {"k": "v"}, "seq": 1}));
    assert_eq!(
        refusal(&mut other, wire),
        (
            "metadata_sequence_source_limit".to_owned(),
            "pane metadata may track at most 32 sequenced sources".to_owned()
        )
    );
}

#[test]
fn a_token_change_publishes_the_complete_pane_record_once() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let wire = report("user:metadata.test-1", json!({"tokens": {"summary": "a"}}));
    accepted(&mut app, wire.clone());
    accepted(&mut app, wire);
    let published: Vec<PaneInfo> = events_after(&app, 0)
        .into_iter()
        .filter_map(|event| match event {
            EventData::PaneUpdated { pane } => Some(pane),
            _ => None,
        })
        .collect();
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].tokens["summary"], "a");
    assert!(published[0].revision > 0);
}

#[test]
fn a_presentation_change_publishes_the_agent_status_with_its_title() {
    let mut app = daemon_with_workspaces(&["/one"]);
    detect(&mut app, Agent::Pi, AgentState::Working);
    accepted(&mut app, titled(json!({"agent": "pi"})));
    let titles: Vec<Option<String>> = events_after(&app, 0)
        .into_iter()
        .filter_map(|event| match event {
            EventData::PaneAgentStatusChanged { title, .. } => Some(title),
            _ => None,
        })
        .collect();
    assert_eq!(titles, [Some("activity".to_owned())]);
}
