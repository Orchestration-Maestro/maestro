//! Pane input through the daemon's handler: right-click routing and the text, key and combined
//! input commands, with the bytes observed by a real shell.
#![cfg(target_os = "linux")]
use maestro_daemon::state::App;
use maestro_daemon::state::test_support::api::{
    daemon_with_workspaces, failure, request, root_pane, split_pane,
};
use maestro_protocol::schema::{ResponseResult, SplitDirection};
use serde_json::json;

#[test]
fn input_to_a_pane_that_cannot_be_written_to_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    for command in ["pane.send_text", "pane.send_keys", "pane.send_input"] {
        for pane in ["w1:p1", "w1:p9"] {
            let wire = json!({"type": command, "pane_id": pane, "text": "x", "keys": ["a"]});
            assert_eq!(
                request(&mut app, wire),
                failure("pane_not_found", &format!("pane {pane} not found")),
                "{command} {pane}"
            );
        }
    }
}
#[test]
fn input_routing_changes_only_the_named_pane() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let target = root_pane(&app, 0);
    let other = split_pane(&mut app, 0, target, SplitDirection::Right).unwrap();
    let routed = |app: &App, pane| {
        app.state.topology.workspaces[0]
            .pane_state(pane)
            .unwrap()
            .right_click_passthrough
    };
    let wire = json!({"type": "pane.input.set", "pane_id": "w1:p1", "right_click": "pane"});
    assert_eq!(request(&mut app, wire), Ok(ResponseResult::Ok {}));
    assert!(routed(&app, target));
    assert!(!routed(&app, other));
    let back = json!({"type": "pane.input.set", "pane_id": "w1:p1", "right_click": "maestro"});
    assert_eq!(request(&mut app, back), Ok(ResponseResult::Ok {}));
    assert!(!routed(&app, target));
    let missing = json!({"type": "pane.input.set", "pane_id": "w1:p9", "right_click": "pane"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane w1:p9 not found")
    );
}
