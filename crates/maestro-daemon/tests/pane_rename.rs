//! Pane renaming through the daemon's handler: the label and its publication.
#![cfg(target_os = "linux")]
use maestro_daemon::state::App;
use maestro_daemon::state::test_support::api::{
    daemon_with_workspaces, events_after, failure, request,
};
use maestro_daemon::state::test_support::pane_rename::rename;
use maestro_protocol::{events::EventData, schema::PaneInfo};
use serde_json::json;
/// The records of the panes published as updated, in order.
fn updated_panes(app: &App) -> Vec<PaneInfo> {
    events_after(app, 0)
        .into_iter()
        .filter_map(|event| match event {
            EventData::PaneUpdated { pane } => Some(pane),
            _ => None,
        })
        .collect()
}
#[test]
fn a_rename_publishes_the_complete_pane_record_only_when_the_label_changes() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    request(&mut app, rename(Some("reviewer"))).unwrap();
    let published = updated_panes(&app);
    assert_eq!(published.len(), 1);
    assert_eq!(published[0].pane_id, "w1:p1");
    assert_eq!(published[0].label.as_deref(), Some("reviewer"));
    request(&mut app, rename(Some(" reviewer "))).unwrap();
    assert_eq!(updated_panes(&app).len(), 1);
    request(&mut app, rename(None)).unwrap();
    let published = updated_panes(&app);
    assert_eq!(published.len(), 2);
    assert_eq!(published[1].label, None);
}
#[test]
fn renaming_a_missing_pane_is_refused_without_a_change() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let wire = json!({"type": "pane.rename", "pane_id": "w1:p9", "label": "x"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_not_found", "pane w1:p9 not found")
    );
    assert!(updated_panes(&app).is_empty());
}
