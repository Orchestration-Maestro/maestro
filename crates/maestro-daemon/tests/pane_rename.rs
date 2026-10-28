//! Pane renaming through the daemon's handler: the label and its publication.
#![cfg(target_os = "linux")]
use crate::support::api::{TestApp, daemon_with_workspaces, events_after, failure, request};
use maestro_protocol::{
    events::EventData,
    schema::{ErrorBody, PaneInfo, ResponseResult},
};
use serde_json::{Value, json};

/// The label an answered rename reports.
fn label_of(result: Result<ResponseResult, ErrorBody>) -> Option<String> {
    match result {
        Ok(ResponseResult::PaneInfo { pane }) => pane.label,
        other => panic!("expected a pane: {other:?}"),
    }
}

/// A rename request for the first pane of the first workspace; `None` leaves the label out.
fn rename(label: Option<&str>) -> Value {
    let mut wire = json!({"type": "pane.rename", "pane_id": "w1:p1"});
    if let Some(label) = label {
        wire["label"] = json!(label);
    }
    wire
}

/// The records of the panes published as updated, in order.
fn updated_panes(app: &TestApp) -> Vec<PaneInfo> {
    events_after(app, 0)
        .into_iter()
        .filter_map(|event| match event {
            EventData::PaneUpdated { pane } => Some(pane),
            _ => None,
        })
        .collect()
}

#[test]
fn renaming_sets_trims_and_clears_the_label() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let retained = |app: &TestApp| {
        let id = app.state.topology.workspaces[0]
            .terminal_id(app.state.topology.workspaces[0].tabs[0].root_pane)
            .unwrap();
        app.state.terminals[id].manual_label.clone()
    };
    app.state.session_dirty = false;
    let named = label_of(request(&mut app, rename(Some("reviewer"))));
    assert_eq!(named.as_deref(), Some("reviewer"));
    assert_eq!(retained(&app).as_deref(), Some("reviewer"));
    assert!(app.state.session_dirty);
    let padded = label_of(request(&mut app, rename(Some("  padded \t"))));
    assert_eq!(padded.as_deref(), Some("padded"));
    assert_eq!(label_of(request(&mut app, rename(Some("   ")))), None);
    assert_eq!(retained(&app), None);
    request(&mut app, rename(Some("again"))).unwrap();
    assert_eq!(label_of(request(&mut app, rename(None))), None);
    assert_eq!(retained(&app), None);
}

#[test]
fn a_rename_publishes_the_complete_pane_record_only_when_the_label_changes() {
    let mut app = daemon_with_workspaces(&["/one"]);
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
    let mut app = daemon_with_workspaces(&["/one"]);
    let wire = json!({"type": "pane.rename", "pane_id": "w1:p9", "label": "x"});
    assert_eq!(
        request(&mut app, wire),
        failure("pane_not_found", "pane w1:p9 not found")
    );
    assert!(updated_panes(&app).is_empty());
}
