//! Pane split through the daemon's handler: targets, focus, ratio, launch inputs and publication.
#![cfg(target_os = "linux")]
use maestro_daemon::state::test_support::api::{daemon_with_workspaces, events_after, request};
use serde_json::json;

#[test]
fn a_split_rejects_a_bad_launch_environment_before_a_process_starts() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let wire = json!({
        "type": "pane.split", "direction": "right", "env": {"BAD=KEY": "value"}
    });
    let Err(error) = request(&mut app, wire) else {
        panic!("a key with an equals sign must be refused");
    };
    assert_eq!(error.code, "invalid_env");
    assert_eq!(
        app.state.topology.workspaces[0].tabs[0].layout.pane_count(),
        1
    );
    assert!(events_after(&app, 0).is_empty());
}
