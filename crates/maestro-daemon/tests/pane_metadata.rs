//! Pane metadata reports through the daemon's handler: validation, tokens, presentation,
//! sequences and publication.
#![cfg(target_os = "linux")]
use maestro_daemon::state::App;
use maestro_daemon::state::test_support::api::{
    accepted, daemon_with_workspaces, detect, events_after, pane_record, pilot, request,
};
use maestro_daemon::state::test_support::pane_metadata::{report, titled, tokens};
use maestro_detection::agents::{Agent, AgentState};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, PaneInfo},
};
use serde_json::{Value, json};
use std::collections::HashMap;
/// The longest lifetime a report may ask for, in milliseconds.
const MAX_TTL_MS: u64 = 86_400_000;
/// What the failure of `wire` is, by code and message.
fn refusal(app: &mut App, wire: Value) -> (String, String) {
    let text = wire.to_string();
    match request(app, wire) {
        Err(ErrorBody { code, message }) => (code, message),
        other => panic!("{text} should be refused: {other:?}"),
    }
}
#[test]
fn tokens_ignore_the_agent_guard_of_the_presentation() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    detect(&mut app, Agent::Claude, AgentState::Working);
    let wire = report(
        "user:metadata.test-1",
        json!({"agent": "codex", "tokens": {"summary": "global"}}),
    )
    .unwrap();
    accepted(&mut app, wire).unwrap();
    assert_eq!(tokens(&mut app).unwrap()["summary"], "global");
    assert_eq!(pane_record(&mut app, "w1:p1").unwrap().title, None);
}
#[test]
fn presentation_and_tokens_share_one_sequence_per_source() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    accepted(&mut app, titled(json!({"seq": 10})).unwrap()).unwrap();
    let stale = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "stale"}, "seq": 9}),
    )
    .unwrap();
    accepted(&mut app, stale).unwrap();
    assert!(tokens(&mut app).unwrap().is_empty());
    let fresh = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "fresh"}, "seq": 11}),
    )
    .unwrap();
    accepted(&mut app, fresh).unwrap();
    assert_eq!(tokens(&mut app).unwrap()["summary"], "fresh");
}
#[test]
fn presentation_fields_reach_the_pane_record_beside_the_tokens() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    pilot(&mut app, AgentState::Idle);
    let session = json!({
        "type": "pane.report_agent_session", "pane_id": "w1:p1", "source": "maestro:pi",
        "agent": "pi", "agent_session_path": "/sessions/pi.jsonl",
        "session_start_source": "startup", "seq": 1
    });
    accepted(&mut app, session).unwrap();
    let hook = json!({
        "type": "pane.report_agent", "pane_id": "w1:p1", "source": "maestro:pi", "agent": "pi",
        "state": "working", "agent_session_path": "/sessions/pi.jsonl", "seq": 2
    });
    accepted(&mut app, hook).unwrap();
    let wire = json!({
        "type": "pane.report_metadata", "pane_id": "w1:p1", "source": "user:pi-display",
        "agent": "pi", "applies_to_source": "maestro:pi", "title": "Refactor auth",
        "display_agent": "Pi auth", "state_labels": {"working": "deep in the mines"},
        "tokens": {"summary": "reviewing auth", "model": "opus"}
    });
    accepted(&mut app, wire).unwrap();
    let reported = pane_record(&mut app, "w1:p1").unwrap();
    assert_eq!(reported.agent.as_deref(), Some("pi"));
    assert_eq!(reported.title.as_deref(), Some("Refactor auth"));
    assert_eq!(reported.display_agent.as_deref(), Some("Pi auth"));
    assert_eq!(reported.state_labels["working"], "deep in the mines");
    assert_eq!(reported.tokens["summary"], "reviewing auth");
    assert_eq!(reported.tokens["model"], "opus");
}
#[test]
fn text_is_cleaned_and_state_labels_are_matched_without_case() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    detect(&mut app, Agent::Pi, AgentState::Idle);
    let long = format!("  tab\there {}  ", "x".repeat(100));
    let wire = titled(json!({
        "agent": "pi", "title": long, "state_labels": {" Working ": "  busy\u{7}bee "}
    }))
    .unwrap();
    accepted(&mut app, wire).unwrap();
    let reported = pane_record(&mut app, "w1:p1").unwrap();
    let title = reported.title.unwrap();
    assert!(
        title.starts_with("tabhere xxxx") && title.chars().count() == 80,
        "{title:?}"
    );
    assert_eq!(reported.state_labels["working"], "busybee");
}
#[test]
fn presentation_text_and_token_values_are_cleaned_alike() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    detect(&mut app, Agent::Pi, AgentState::Idle);
    let cases = [
        (
            format!("  tab\there {}  ", "x".repeat(100)),
            format!("tabhere {}", "x".repeat(72)),
        ),
        (format!("{} tail", "a".repeat(79)), "a".repeat(79)),
    ];
    for (raw, kept) in cases {
        let wire =
            titled(json!({"agent": "pi", "title": raw, "tokens": {"summary": raw}})).unwrap();
        accepted(&mut app, wire).unwrap();
        let reported = pane_record(&mut app, "w1:p1").unwrap();
        assert_eq!(reported.title.as_deref(), Some(kept.as_str()), "{raw:?}");
        assert_eq!(reported.tokens["summary"], kept, "{raw:?}");
    }
    let blank = " \u{7}\n ";
    let wire =
        titled(json!({"agent": "pi", "title": blank, "tokens": {"summary": blank}})).unwrap();
    accepted(&mut app, wire).unwrap();
    let reported = pane_record(&mut app, "w1:p1").unwrap();
    assert_eq!(reported.title, Some("a".repeat(79)));
    assert!(reported.tokens.is_empty());
}
#[test]
fn a_documented_source_and_the_longest_lifetime_are_accepted() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    accepted(&mut app, titled(json!({"ttl_ms": MAX_TTL_MS})).unwrap()).unwrap();
}
#[test]
fn a_malformed_source_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let long = "a".repeat(81);
    for source in [
        "",
        "   ",
        "user metadata",
        "user/metadata",
        "user:\u{7f}metadata",
        &long,
    ] {
        let (code, _) = refusal(&mut app, titled(json!({"source": source})).unwrap());
        assert_eq!(code, "invalid_metadata_source", "{source:?}");
    }
    for guard in ["herdr source", "   "] {
        let wire = titled(json!({"applies_to_source": guard})).unwrap();
        assert_eq!(
            refusal(&mut app, wire).0,
            "invalid_metadata_source",
            "{guard:?}"
        );
    }
}
#[test]
fn a_lifetime_outside_the_supported_range_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    for ttl in [0, MAX_TTL_MS + 1] {
        let (code, _) = refusal(&mut app, titled(json!({"ttl_ms": ttl})).unwrap());
        assert_eq!(code, "invalid_metadata_ttl", "{ttl}");
    }
}
#[test]
fn contradictory_or_empty_requests_are_refused() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let conflict = "cannot set and clear the same metadata field";
    for wire in [
        titled(json!({"clear_title": true})).unwrap(),
        titled(json!({"title": "   ", "clear_title": true})).unwrap(),
        titled(json!({"title": null, "display_agent": "x", "clear_display_agent": true})).unwrap(),
        titled(json!({"title": null, "state_labels": {"idle": "x"}, "clear_state_labels": true}))
            .unwrap(),
    ] {
        let expected = ("invalid_metadata_request".to_owned(), conflict.to_owned());
        assert_eq!(refusal(&mut app, wire.clone()), expected, "{wire}");
    }
    let empty = report("user:metadata.test-1", json!({})).unwrap();
    let expected = (
        "invalid_metadata_request".to_owned(),
        "missing metadata field to set or clear".to_owned(),
    );
    assert_eq!(refusal(&mut app, empty), expected);
}
#[test]
fn an_unknown_state_label_and_a_malformed_token_are_refused() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let sleepy = titled(json!({"state_labels": {"sleepy": "zzz"}})).unwrap();
    assert_eq!(
        refusal(&mut app, sleepy),
        (
            "invalid_state_label".to_owned(),
            "unknown state label: sleepy".to_owned()
        )
    );
    let dotted = titled(json!({"tokens": {"bad.name": "v"}})).unwrap();
    assert_eq!(
        refusal(&mut app, dotted),
        (
            "invalid_metadata_token".to_owned(),
            "invalid metadata token key: bad.name".to_owned()
        )
    );
    let many: HashMap<String, Value> = (0..17).map(|n| (format!("key{n}"), json!("v"))).collect();
    let (code, _) = refusal(&mut app, titled(json!({"tokens": many})).unwrap());
    assert_eq!(code, "invalid_metadata_token");
}
#[test]
fn a_pane_holds_at_most_thirty_two_tokens_and_a_source_list_is_bounded() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    for batch in 0..2 {
        let tokens: HashMap<String, Value> = (0..16)
            .map(|n| (format!("key{batch}x{n}"), json!("v")))
            .collect();
        accepted(
            &mut app,
            report("user:fill", json!({"tokens": tokens})).unwrap(),
        )
        .unwrap();
    }
    let one_more = report("user:fill", json!({"tokens": {"overflow": "v"}})).unwrap();
    assert_eq!(
        refusal(&mut app, one_more),
        (
            "metadata_token_limit".to_owned(),
            "pane metadata may contain at most 32 tokens".to_owned()
        )
    );
    assert_eq!(tokens(&mut app).unwrap().len(), 32);
    let mut other = daemon_with_workspaces(&["/two"]).unwrap();
    for source in 0..32 {
        let wire = report(
            &format!("user:s{source}"),
            json!({"tokens": {"k": "v"}, "seq": 1}),
        )
        .unwrap();
        accepted(&mut other, wire).unwrap();
    }
    let wire = report("user:s32", json!({"tokens": {"k": "v"}, "seq": 1})).unwrap();
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
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let wire = report("user:metadata.test-1", json!({"tokens": {"summary": "a"}})).unwrap();
    accepted(&mut app, wire.clone()).unwrap();
    accepted(&mut app, wire).unwrap();
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
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    detect(&mut app, Agent::Pi, AgentState::Working);
    let sequence = app.event_hub().current_sequence();
    accepted(&mut app, titled(json!({"agent": "pi"})).unwrap()).unwrap();
    let titles: Vec<Option<String>> = events_after(&app, sequence)
        .into_iter()
        .filter_map(|event| match event {
            EventData::PaneAgentStatusChanged { title, .. } => Some(title),
            _ => None,
        })
        .collect();
    assert_eq!(titles, [Some("activity".to_owned())]);
}
