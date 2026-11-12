//! The shared test helpers report a missing fixture or a refused command as a value.
use crate::state::test_support::api::{accepted, daemon_with_workspaces, detect, pilot, root_pane};
use crate::state::test_support::daemon_api::tokens;
use crate::state::test_support::pane_metadata::report;
use crate::state::test_support::pane_swap::leaves;
use maestro_detection::agents::{Agent, AgentState};
use serde_json::json;

#[test]
fn accepted_returns_the_refusal_of_a_command_the_daemon_rejects() {
    let mut app = daemon_with_workspaces(&["/issue"]).unwrap();
    let refused = json!({"type": "pane.send_keys", "pane_id": "missing", "keys": ["x"]});
    assert!(accepted(&mut app, refused).is_err());
}

#[test]
fn accepted_returns_ok_for_a_plain_acknowledgement() {
    let mut app = daemon_with_workspaces(&["/issue"]).unwrap();
    let wire = report(
        "user:metadata.test-1",
        json!({"tokens": {"summary": "done"}}),
    )
    .unwrap();
    assert_eq!(accepted(&mut app, wire), Ok(()));
}

#[test]
fn readers_return_none_without_a_workspace_and_some_with_one() {
    let empty = daemon_with_workspaces(&[]).unwrap();
    assert_eq!(root_pane(&empty, 0), None);
    assert_eq!(leaves(&empty), None);
    assert_eq!(tokens(&empty), None);
    let app = daemon_with_workspaces(&["/issue"]).unwrap();
    assert!(root_pane(&app, 0).is_some());
    assert_eq!(root_pane(&app, 1), None);
    assert_eq!(leaves(&app).map(|panes| panes.len()), Some(1));
    assert!(tokens(&app).is_some());
}

#[test]
fn detect_and_pilot_report_a_missing_pane() {
    let mut empty = daemon_with_workspaces(&[]).unwrap();
    assert_eq!(detect(&mut empty, Agent::Pi, AgentState::Idle), None);
    assert_eq!(pilot(&mut empty, AgentState::Idle), None);
}
