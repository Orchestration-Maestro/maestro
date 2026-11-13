//! Tab commands through the daemon's handler: targets, ordering, close and creation.
#![cfg(target_os = "linux")]
use maestro_daemon::state::test_support::api::{
    daemon_with_workspaces, events_after, failure, request, workspace_ids,
};
use serde_json::json;

#[test]
fn launch_environment_rejects_equals_in_keys_before_spawning() {
    let mut app = daemon_with_workspaces(&["/tabs"]).unwrap();
    for create in [
        json!({"type": "tab.create", "env": {"BAD=KEY": "value"}}),
        json!({"type": "workspace.create", "env": {"BAD=KEY": "value"}}),
    ] {
        assert_eq!(
            request(&mut app, create),
            failure("invalid_env", "env key BAD=KEY must not contain '='")
        );
    }
    assert_eq!(workspace_ids(&mut app).unwrap(), ["w1"]);
    assert_eq!(app.state.topology.workspaces[0].tabs.len(), 1);
    assert_eq!(app.state.topology.workspaces[0].next_public_tab_number, 2);
    assert!(events_after(&app, 0).is_empty());
}
