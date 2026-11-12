//! Workspace commands through the daemon's handler: targets, ordering, close and creation.
#![cfg(target_os = "linux")]
use maestro_daemon::state::test_support::api::{
    daemon_with_workspaces, events_after, failure, request, workspace_ids,
};
use maestro_daemon::state::test_support::workspace_commands::listed;
use serde_json::json;

#[test]
fn workspace_block_validation_is_atomic() {
    let mut app = daemon_with_workspaces(&["/one", "/two", "/three", "/four"]).unwrap();
    let block = |ids: serde_json::Value, before: Option<&str>| json!({"type": "workspace.move_block", "workspace_ids": ids, "before_workspace_id": before});
    let failed = |message: &str| failure("workspace_move_block_failed", message);
    for (rejected, expected) in [
        (
            block(json!([]), None),
            failed("workspace_ids must not be empty"),
        ),
        (
            block(json!(["w1", "1"]), None),
            failed("workspace 1 appears more than once"),
        ),
        (
            block(json!(["w_2", "w2"]), None),
            failed("workspace w2 appears more than once"),
        ),
        (
            block(json!(["w1", "w9"]), None),
            failure("workspace_not_found", "workspace w9 not found"),
        ),
        (
            block(json!(["w1"]), Some("w9")),
            failure("workspace_not_found", "workspace w9 not found"),
        ),
        (
            block(json!(["w1", "w3"]), Some("3")),
            failed("before_workspace_id must not be part of workspace_ids"),
        ),
    ] {
        assert_eq!(request(&mut app, rejected.clone()), expected, "{rejected}");
        assert_eq!(workspace_ids(&mut app).unwrap(), ["w1", "w2", "w3", "w4"]);
    }
    assert!(events_after(&app, 0).is_empty());
    for (accepted, order) in [
        (block(json!(["w4"]), Some("w1")), ["w4", "w1", "w2", "w3"]),
        (block(json!(["w4"]), None), ["w1", "w2", "w3", "w4"]),
        (
            block(json!(["w3", "w1"]), Some("w2")),
            ["w3", "w1", "w2", "w4"],
        ),
    ] {
        assert_eq!(listed(&request(&mut app, accepted)).unwrap(), order);
    }
    assert_eq!(events_after(&app, 0).len(), 3);
}
