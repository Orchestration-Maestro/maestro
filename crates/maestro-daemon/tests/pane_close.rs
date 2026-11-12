//! Pane close through the daemon's handler: the pane, its tab or its workspace goes, with its terminal.
#![cfg(target_os = "linux")]
use maestro_daemon::state::test_support::api::{
    daemon_with_workspaces, events_after, failure, workspace_ids,
};
use maestro_daemon::state::test_support::pane_close::close;

#[test]
fn closing_a_missing_pane_is_refused_without_a_change() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    assert_eq!(
        close(&mut app, "w1:p9"),
        failure("pane_not_found", "pane w1:p9 not found")
    );
    assert_eq!(workspace_ids(&mut app).unwrap(), ["w1"]);
    assert!(events_after(&app, 0).is_empty());
}
