//! Layout export and split ratio through the daemon's handler: targets, the portable tree,
//! clamping and publication.

use crate::state::App;
use crate::state::test_support::api::{
    answer_command, daemon_with_workspaces, events_after, failure, request, root_pane, split_pane,
};
use crate::state::test_support::layout_commands::{adjusted, exported, two_tabs};
use crate::state::tests::support::layouts::{balanced, chain, shell, sleeper, split};
use crate::tests::block_on;
use maestro_protocol::{
    envelopes::RpcCommand,
    events::EventData,
    schema::{LayoutApplyParams, LayoutDescription, LayoutNode, LayoutPane, SplitDirection},
};
use maestro_workspace::tiling::PaneId;
use serde_json::json;
/// An exported pane leaf in `cwd` with its public handle.
fn leaf(pane_id: &str, cwd: &str, label: Option<&str>) -> LayoutNode {
    LayoutNode::Pane {
        pane: LayoutPane {
            pane_id: Some(pane_id.into()),
            label: label.map(Into::into),
            cwd: Some(cwd.into()),
            ..LayoutPane::default()
        },
    }
}
/// The terminal of `pane` in the first workspace.
fn terminal_of(app: &App, pane: PaneId) -> maestro_workspace::terminal_id::TerminalId {
    app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone()
}
#[test]
fn export_describes_the_focused_tab_as_a_portable_tree() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let root = root_pane(&app, 0).unwrap();
    let right = split_pane(&mut app, 0, root, SplitDirection::Right).unwrap();
    let layout = &mut app.state.topology.workspaces[0].tabs[0].layout;
    layout.focus_pane(root);
    assert!(layout.set_ratio_at(&[], 0.65));
    let terminal = terminal_of(&app, right);
    app.state
        .terminals
        .get_mut(&terminal)
        .unwrap()
        .set_manual_label("tests");
    assert_eq!(
        exported(request(&mut app, json!({"type": "layout.export"}))).unwrap(),
        LayoutDescription {
            workspace_id: "w1".into(),
            tab_id: "w1:t1".into(),
            zoomed: false,
            focused_pane_id: "w1:p1".into(),
            root: LayoutNode::Split {
                direction: SplitDirection::Right,
                ratio: 0.65,
                first: Box::new(leaf("w1:p1", "/one", None)),
                second: Box::new(leaf("w1:p2", "/one", Some("tests"))),
            },
        }
    );
}
#[test]
fn export_carries_the_zoom_and_the_launch_command_of_a_pane() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let root = root_pane(&app, 0).unwrap();
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    let terminal = terminal_of(&app, root);
    app.state.terminals.get_mut(&terminal).unwrap().launch_argv =
        Some(vec!["just".into(), "dev".into()]);
    let layout = exported(request(&mut app, json!({"type": "layout.export"}))).unwrap();
    assert!(layout.zoomed);
    let LayoutNode::Pane { pane } = layout.root else {
        panic!("a single pane exports as a leaf");
    };
    assert_eq!(pane.command, Some(vec!["just".into(), "dev".into()]));
    assert!(pane.env.is_empty());
}
#[test]
fn export_refuses_a_target_that_is_missing_or_ambiguous() {
    let (mut app, _, _) = two_tabs().unwrap();
    for wire in [
        json!({"type": "layout.export", "tab_id": "w1:t9"}),
        json!({"type": "layout.export", "pane_id": "w1:p9"}),
        json!({"type": "layout.export", "tab_id": "w1:t1", "pane_id": "w1:p1"}),
    ] {
        assert_eq!(
            request(&mut app, wire.clone()),
            failure("layout_not_found", "layout target not found"),
            "{wire}"
        );
    }
    app.state.active = None;
    assert_eq!(
        request(&mut app, json!({"type": "layout.export"})),
        failure("layout_not_found", "layout target not found")
    );
}
#[test]
fn a_split_ratio_change_is_answered_and_published() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let root = root_pane(&app, 0).unwrap();
    split_pane(&mut app, 0, root, SplitDirection::Right).unwrap();
    let wire = json!({"type": "layout.set_split_ratio", "path": [], "ratio": 0.72});
    let layout = adjusted(request(&mut app, wire)).unwrap();
    let LayoutNode::Split { ratio, .. } = layout.root else {
        panic!("the tab is split");
    };
    assert!((ratio - 0.72).abs() < f32::EPSILON);
    assert!(app.state.session_dirty);
    let events = events_after(&app, 0);
    assert!(
        matches!(
            events.as_slice(),
            [EventData::LayoutUpdated { layout }]
                if layout.tab_id == "w1:t1" && (layout.splits[0].ratio - 0.72).abs() < f32::EPSILON
        ),
        "{events:?}"
    );
}
#[test]
fn a_split_ratio_for_a_missing_split_is_refused_without_a_change() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    for path in [json!([]), json!([true])] {
        let wire = json!({"type": "layout.set_split_ratio", "path": path, "ratio": 0.72});
        assert_eq!(
            request(&mut app, wire),
            failure("split_not_found", "split path not found"),
            "{path}"
        );
    }
    let root = root_pane(&app, 0).unwrap();
    split_pane(&mut app, 0, root, SplitDirection::Right).unwrap();
    let wire = json!({"type": "layout.set_split_ratio", "path": [true], "ratio": 0.72});
    assert_eq!(
        request(&mut app, wire),
        failure("split_not_found", "split path not found")
    );
    assert!(!app.state.session_dirty);
    assert!(events_after(&app, 0).is_empty());
}
/// The tab count, terminal count and event count that a refused apply must leave as they were.
fn footprint(app: &mut App) -> (usize, usize, usize, bool) {
    (
        app.state.topology.workspaces[0].tabs.len(),
        app.state.terminals.len(),
        events_after(app, 0).len(),
        app.state.session_dirty,
    )
}
#[test]
fn applying_to_a_target_that_is_missing_or_ambiguous_is_refused_before_a_process_starts() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let before = footprint(&mut app);
    for (target, code, message) in [
        (
            json!({"tab_id": "w1:t9"}),
            "tab_not_found",
            "tab w1:t9 not found",
        ),
        (
            json!({"tab_id": "w1:t9", "workspace_id": "w9"}),
            "tab_not_found",
            "tab w1:t9 not found",
        ),
        (
            json!({"workspace_id": "w9"}),
            "workspace_not_found",
            "workspace w9 not found",
        ),
        (
            json!({"tab_id": "w1:t1", "workspace_id": "w1"}),
            "invalid_target",
            "use either tab_id or workspace_id, not both",
        ),
    ] {
        let mut wire = json!({"type": "layout.apply", "root": shell()});
        wire.as_object_mut()
            .unwrap()
            .extend(target.as_object().unwrap().clone());
        assert_eq!(request(&mut app, wire), failure(code, message), "{target}");
    }
    app.state.active = None;
    assert_eq!(
        request(&mut app, json!({"type": "layout.apply", "root": shell()})),
        failure("workspace_not_found", "no active workspace")
    );
    assert_eq!(footprint(&mut app), before);
}
#[test]
fn a_leaf_that_cannot_be_launched_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let before = footprint(&mut app);
    let empty_command = json!({"type": "pane", "command": []});
    let bad_env = json!({"type": "pane", "env": {"BAD=KEY": "value"}});
    for (root, message) in [
        (
            split(shell(), empty_command.clone()),
            "pane command must not be empty",
        ),
        (empty_command, "pane command must not be empty"),
        (
            split(bad_env.clone(), shell()),
            "env key BAD=KEY must not contain '='",
        ),
        (
            split(shell(), split(shell(), bad_env)),
            "env key BAD=KEY must not contain '='",
        ),
    ] {
        let wire = json!({"type": "layout.apply", "root": root});
        assert_eq!(
            request(&mut app, wire),
            failure("invalid_layout", message),
            "{message}"
        );
    }
    assert_eq!(footprint(&mut app), before);
}
#[test]
fn a_layout_split_that_is_not_finite_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let before = footprint(&mut app);
    let leaf = || LayoutNode::Pane {
        pane: LayoutPane::default(),
    };
    let command = RpcCommand::LayoutApply(LayoutApplyParams {
        workspace_id: None,
        tab_id: None,
        tab_label: None,
        focus: false,
        root: LayoutNode::Split {
            direction: SplitDirection::Right,
            ratio: f32::NAN,
            first: Box::new(leaf()),
            second: Box::new(leaf()),
        },
    });
    assert_eq!(
        block_on(answer_command(&mut app, command)),
        failure("invalid_layout", "split ratio must be finite")
    );
    assert_eq!(footprint(&mut app), before);
}
#[test]
fn a_layout_deeper_than_sixteen_levels_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let before = footprint(&mut app);
    for leaves in [17, 20] {
        let wire = json!({"type": "layout.apply", "root": chain(leaves, &sleeper())});
        assert_eq!(
            request(&mut app, wire),
            failure("invalid_layout", "layout depth is 17; maximum is 16"),
            "{leaves}"
        );
    }
    assert_eq!(footprint(&mut app), before);
}
#[test]
fn a_layout_of_more_than_twenty_four_panes_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]).unwrap();
    let before = footprint(&mut app);
    let wire = json!({"type": "layout.apply", "root": balanced(25, &sleeper())});
    assert_eq!(
        request(&mut app, wire),
        failure("invalid_layout", "layout has more than 24 panes")
    );
    assert_eq!(footprint(&mut app), before);
}
