//! The shared test helpers report a missing fixture or a refused command as a value.
use crate::state::test_support::api::{accepted, daemon_with_workspaces, root_pane};
use crate::state::test_support::daemon_api::tokens;
use crate::state::test_support::pane_metadata::report;
use crate::state::test_support::pane_swap::leaves;
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
fn readers_of_a_missing_workspace_return_none() {
    let app = daemon_with_workspaces(&["/issue"]).unwrap();
    assert!(root_pane(&app, 0).is_some());
    assert_eq!(root_pane(&app, 1), None);
    assert_eq!(leaves(&app).map(|panes| panes.len()), Some(1));
    assert!(tokens(&app).is_some());
}
