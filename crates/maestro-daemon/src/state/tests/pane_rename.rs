//! Pane renaming through the daemon's handler: the label and its publication.
use crate::state::App;
use crate::state::test_support::api::{daemon_with_workspaces, pane_record, request};
use crate::state::test_support::pane_rename::rename;
use maestro_protocol::schema::{ErrorBody, ResponseResult};
/// The label an answered rename reports.
fn label_of(result: Result<ResponseResult, ErrorBody>) -> Option<String> {
    match result {
        Ok(ResponseResult::PaneInfo { pane }) => pane.label,
        other => panic!("expected a pane: {other:?}"),
    }
}
#[test]
fn renaming_sets_trims_and_clears_the_label() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let retained = |app: &mut App| pane_record(app, "w1:p1").unwrap().label;
    app.state.session_dirty = false;
    let named = label_of(request(&mut app, rename(Some("reviewer"))));
    assert_eq!(named.as_deref(), Some("reviewer"));
    assert_eq!(retained(&mut app).as_deref(), Some("reviewer"));
    assert!(app.state.session_dirty);
    let padded = label_of(request(&mut app, rename(Some("  padded \t"))));
    assert_eq!(padded.as_deref(), Some("padded"));
    assert_eq!(label_of(request(&mut app, rename(Some("   ")))), None);
    assert_eq!(retained(&mut app), None);
    request(&mut app, rename(Some("again"))).unwrap();
    assert_eq!(label_of(request(&mut app, rename(None))), None);
    assert_eq!(retained(&mut app), None);
}
