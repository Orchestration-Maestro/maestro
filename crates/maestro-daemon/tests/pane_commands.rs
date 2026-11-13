//! Pane topology commands through the daemon's handler: reads, focus and neighbours.
#![cfg(target_os = "linux")]
use maestro_daemon::state::App;
use maestro_daemon::state::test_support::api::{add_tab, add_workspaces, failure, pair, request};
use maestro_daemon::state::test_support::pane_commands::{focus_toward, listed, public, reported};
use maestro_protocol::schema::{
    ErrorBody, PaneDirection, PaneFocusDirectionReason, PaneLayoutSnapshot, ResponseResult,
};
use serde_json::json;
/// The layout a layout result reports.
fn layout_of(result: Result<ResponseResult, ErrorBody>) -> PaneLayoutSnapshot {
    match result {
        Ok(ResponseResult::PaneLayout { layout }) => layout,
        other => panic!("expected a pane layout: {other:?}"),
    }
}
#[test]
fn pane_listing_follows_workspace_tab_and_split_order() {
    let (mut app, _, _) = pair().unwrap();
    add_tab(&mut app, 0).unwrap();
    add_workspaces(&mut app, &["/two"]).unwrap();
    assert_eq!(
        listed(request(&mut app, json!({"type": "pane.list"}))).unwrap(),
        ["w1:p1", "w1:p2", "w1:p3", "w2:p1"]
    );
    for alias in ["w2", "w_2", "2"] {
        let list = json!({"type": "pane.list", "workspace_id": alias});
        assert_eq!(
            listed(request(&mut app, list)).unwrap(),
            ["w2:p1"],
            "{alias}"
        );
    }
    let list = json!({"type": "pane.list", "workspace_id": "w9"});
    assert_eq!(
        request(&mut app, list),
        failure("workspace_not_found", "workspace w9 not found")
    );
}
#[test]
fn the_current_pane_prefers_the_caller_then_the_focus() {
    let (mut app, left, right) = pair().unwrap();
    let focused = reported(request(&mut app, json!({"type": "pane.current"}))).unwrap();
    assert_eq!(focused.pane_id, public(&app, 0, left).unwrap());
    assert!(focused.focused);
    let caller = json!({"type": "pane.current", "caller_pane_id": "w1:p2"});
    let named = reported(request(&mut app, caller)).unwrap();
    assert_eq!(named.pane_id, public(&app, 0, right).unwrap());
    assert!(!named.focused);
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}
#[test]
fn a_pane_is_read_by_any_accepted_spelling_and_a_missing_one_is_refused() {
    let (mut app, _, right) = pair().unwrap();
    for alias in ["w1:p2", "w1-2"] {
        let get = json!({"type": "pane.get", "pane_id": alias});
        assert_eq!(
            reported(request(&mut app, get)).unwrap().pane_id,
            public(&app, 0, right).unwrap()
        );
    }
    let get = json!({"type": "pane.get", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, get),
        failure("pane_not_found", "pane w1:p9 not found")
    );
}
#[test]
fn focusing_a_missing_pane_is_refused_without_a_change() {
    let (mut app, left, _) = pair().unwrap();
    let focus = json!({"type": "pane.focus", "pane_id": "pane_missing"});
    assert_eq!(
        request(&mut app, focus),
        failure("pane_not_found", "pane pane_missing not found")
    );
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}
#[test]
fn a_layout_describes_the_tab_of_the_named_or_focused_pane() {
    let (mut app, _, _) = pair().unwrap();
    add_tab(&mut app, 0).unwrap();
    let expected = app.pane_layout_snapshot(0, 0).unwrap();
    let named = json!({"type": "pane.layout", "pane_id": "w1:p2"});
    assert_eq!(layout_of(request(&mut app, named)), expected);
    assert_eq!(
        layout_of(request(&mut app, json!({"type": "pane.layout"}))),
        expected
    );
    let other = json!({"type": "pane.layout", "pane_id": "w1:p3"});
    assert_eq!(
        layout_of(request(&mut app, other)),
        app.pane_layout_snapshot(0, 1).unwrap()
    );
    let missing = json!({"type": "pane.layout", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane not found")
    );
}
/// The neighbour record a neighbour result holds.
fn neighbour_of(app: &mut App, pane: &str, direction: &str) -> Option<String> {
    let command = json!({"type": "pane.neighbor", "pane_id": pane, "direction": direction});
    let Ok(ResponseResult::PaneNeighbor { neighbor }) = request(app, command) else {
        panic!("expected a neighbour");
    };
    assert_eq!(neighbor.pane_id, pane);
    assert_eq!(neighbor.layout, app.pane_layout_snapshot(0, 0).unwrap());
    neighbor.neighbor_pane_id
}
#[test]
fn a_neighbour_is_the_pane_next_to_it_in_a_direction() {
    let (mut app, _, _) = pair().unwrap();
    assert_eq!(
        neighbour_of(&mut app, "w1:p1", "right").as_deref(),
        Some("w1:p2")
    );
    assert_eq!(
        neighbour_of(&mut app, "w1:p2", "left").as_deref(),
        Some("w1:p1")
    );
    assert_eq!(neighbour_of(&mut app, "w1:p1", "left"), None);
    assert_eq!(neighbour_of(&mut app, "w1:p1", "up"), None);
}
#[test]
fn edges_tell_which_sides_of_the_view_a_pane_touches() {
    let (mut app, _, _) = pair().unwrap();
    for (pane, left, right) in [("w1:p1", true, false), ("w1:p2", false, true)] {
        let command = json!({"type": "pane.edges", "pane_id": pane});
        let Ok(ResponseResult::PaneEdges { edges }) = request(&mut app, command) else {
            panic!("expected edges");
        };
        assert_eq!(edges.pane_id, pane);
        assert_eq!((edges.left, edges.right), (left, right), "{pane}");
        assert!(edges.up && edges.down.0, "{pane}");
        assert_eq!(edges.layout, app.pane_layout_snapshot(0, 0).unwrap());
    }
    let missing = json!({"type": "pane.edges", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane not found")
    );
}
#[test]
fn focus_stays_put_when_no_neighbour_lies_that_way() {
    let (mut app, left, _) = pair().unwrap();
    let focus = focus_toward(&mut app, "w1:p1", PaneDirection::Left).unwrap();
    assert_eq!(focus["changed"], false);
    assert_eq!(
        focus["reason"],
        serde_json::to_value(PaneFocusDirectionReason::NoNeighbor).unwrap()
    );
    assert_eq!(focus["source_pane_id"], "w1:p1");
    assert_eq!(focus["focused_pane_id"], "w1:p1");
    assert_eq!(
        app.state.topology.workspaces[0].focused_pane_id(),
        Some(left)
    );
}
#[test]
fn process_info_needs_a_pane_with_a_process() {
    let (mut app, _, _) = pair().unwrap();
    let info = json!({"type": "pane.process_info", "pane_id": "w1:p1"});
    assert_eq!(
        request(&mut app, info),
        failure("pane_not_found", "pane not found")
    );
    let missing = json!({"type": "pane.process_info", "pane_id": "w1:p9"});
    assert_eq!(
        request(&mut app, missing),
        failure("pane_not_found", "pane not found")
    );
}
