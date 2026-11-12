//! Pane reads through the daemon's handler: the shared snapshot clipping, sources, formats and
//! the record a read answers with.
#![cfg(target_os = "linux")]
use maestro_daemon::panes::display::TerminalReadSnapshot;
use maestro_daemon::state::test_support::api::{daemon_with_workspaces, failure, request};
use serde_json::json;

#[test]
fn snapshot_limits_keep_endings_and_report_truncation() {
    let snapshot = TerminalReadSnapshot::limited("one\ntwø\n三\n".into(), Some(2));
    assert_eq!(snapshot.text, "twø\n三\n");
    assert!(snapshot.truncated);
    let snapshot = TerminalReadSnapshot::limited("one\ntwo\nthree".into(), Some(1));
    assert_eq!(snapshot.text, "three");
    assert!(snapshot.truncated);
    let snapshot = TerminalReadSnapshot::limited("one\ntwo".into(), Some(0));
    assert_eq!(snapshot.text, "");
    assert!(snapshot.truncated);
    let snapshot = TerminalReadSnapshot::limited(String::new(), Some(2));
    assert_eq!(snapshot.text, "");
    assert!(!snapshot.truncated);
}
#[test]
fn an_absent_snapshot_limit_keeps_all_text() {
    let snapshot = TerminalReadSnapshot::limited("one\ntwo\n".into(), None);
    assert_eq!(snapshot.text, "one\ntwo\n");
    assert!(!snapshot.truncated);
}
#[test]
fn a_pane_that_cannot_be_read_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    for pane in ["w1:p1", "w1:p9"] {
        let wire = json!({"type": "pane.read", "pane_id": pane, "source": "visible"});
        assert_eq!(
            request(&mut app, wire),
            failure("pane_not_found", &format!("pane {pane} not found"))
        );
    }
}
