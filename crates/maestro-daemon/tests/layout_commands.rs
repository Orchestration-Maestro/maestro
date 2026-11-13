//! Layout export and split ratio through the daemon's handler: targets, the portable tree,
//! clamping and publication.
#![cfg(target_os = "linux")]

use maestro_daemon::state::test_support::api::{
    answer_command, daemon_with_workspaces, events_after, failure, request, root_pane, split_pane,
};
use maestro_daemon::state::test_support::layout_commands::{adjusted, exported, two_tabs};
use maestro_daemon::state::test_support::run;
use maestro_protocol::{
    envelopes::RpcCommand,
    events::EventData,
    schema::{LayoutNode, LayoutSetSplitRatioParams, SplitDirection},
};
use serde_json::json;

#[test]
fn export_names_its_tab_directly_or_through_one_of_its_panes() {
    let (mut app, _, _) = two_tabs().unwrap();
    let default = exported(request(&mut app, json!({"type": "layout.export"}))).unwrap();
    assert_eq!(default.tab_id, "w1:t1");
    for wire in [
        json!({"type": "layout.export", "tab_id": "w1:t2"}),
        json!({"type": "layout.export", "pane_id": "w1:p3"}),
    ] {
        let layout = exported(request(&mut app, wire.clone())).unwrap();
        assert_eq!(layout.tab_id, "w1:t2", "{wire}");
        assert_eq!(layout.focused_pane_id, "w1:p2", "{wire}");
        assert!(matches!(layout.root, LayoutNode::Split { .. }), "{wire}");
    }
}
#[test]
fn a_split_ratio_change_reaches_a_nested_split_by_its_path() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let root = root_pane(&app, 0).unwrap();
    let right = split_pane(&mut app, 0, root, SplitDirection::Right).unwrap();
    split_pane(&mut app, 0, right, SplitDirection::Down).unwrap();
    let wire = json!({"type": "layout.set_split_ratio", "path": [true], "ratio": 0.3});
    let layout = adjusted(request(&mut app, wire)).unwrap();
    let LayoutNode::Split { ratio, second, .. } = layout.root else {
        panic!("the tab is split");
    };
    let LayoutNode::Split { ratio: nested, .. } = *second else {
        panic!("the second child is split");
    };
    assert!((ratio - 0.5).abs() < f32::EPSILON);
    assert!((nested - 0.3).abs() < f32::EPSILON);
}
#[test]
fn a_split_ratio_is_kept_within_the_range_a_split_allows() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let root = root_pane(&app, 0).unwrap();
    split_pane(&mut app, 0, root, SplitDirection::Right).unwrap();
    for (requested, kept) in [(5.0, 0.9), (-1.0, 0.1)] {
        let wire = json!({"type": "layout.set_split_ratio", "path": [], "ratio": requested});
        let LayoutNode::Split { ratio, .. } = adjusted(request(&mut app, wire)).unwrap().root
        else {
            panic!("the tab is split");
        };
        assert!((ratio - kept).abs() < f32::EPSILON, "{requested}");
    }
}
#[test]
fn a_split_ratio_that_is_not_finite_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let root = root_pane(&app, 0).unwrap();
    split_pane(&mut app, 0, root, SplitDirection::Right).unwrap();
    for ratio in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let command = RpcCommand::LayoutSetSplitRatio(LayoutSetSplitRatioParams {
            tab_id: None,
            pane_id: None,
            path: Vec::new(),
            ratio,
        });
        let result = run(answer_command(&mut app, command)).unwrap();
        assert_eq!(
            result,
            failure("invalid_ratio", "ratio must be finite"),
            "{ratio}"
        );
    }
    assert!(events_after(&app, 0).is_empty());
}
#[test]
fn a_split_ratio_names_its_tab_directly_or_through_one_of_its_panes() {
    let (mut app, _, _) = two_tabs().unwrap();
    for (target, tab) in [
        (json!({"tab_id": "w1:t2"}), "w1:t2"),
        (json!({"pane_id": "w1:p3"}), "w1:t2"),
    ] {
        let mut wire = json!({"type": "layout.set_split_ratio", "path": [], "ratio": 0.4});
        wire.as_object_mut()
            .unwrap()
            .extend(target.as_object().unwrap().clone());
        assert_eq!(
            adjusted(request(&mut app, wire)).unwrap().tab_id,
            tab,
            "{target}"
        );
    }
    let published = events_after(&app, 0);
    assert!(
        published.iter().all(|event| matches!(
            event,
            EventData::LayoutUpdated { layout } if layout.tab_id == "w1:t2"
        )) && published.len() == 2,
        "{published:?}"
    );
    for wire in [
        json!({"type": "layout.set_split_ratio", "path": [], "ratio": 0.4, "tab_id": "w1:t9"}),
        json!({
            "type": "layout.set_split_ratio", "path": [], "ratio": 0.4,
            "tab_id": "w1:t1", "pane_id": "w1:p1"
        }),
    ] {
        assert_eq!(
            request(&mut app, wire.clone()),
            failure("layout_not_found", "layout target not found"),
            "{wire}"
        );
    }
}
