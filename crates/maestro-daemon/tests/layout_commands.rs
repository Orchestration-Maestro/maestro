//! Layout export and split ratio through the daemon's handler: targets, the portable tree,
//! clamping and publication.
use crate::support::{
    api::{
        TestApp, add_tab, answer_command, daemon_with_workspaces, events_after, failure, request,
        root_pane, split_pane,
    },
    layouts::{balanced, chain, shell, sleeper, split},
    runtime::block_on,
};
use maestro_protocol::{
    events::EventData,
    rpc_types::RpcCommand,
    schema::{
        ErrorBody, LayoutApplyParams, LayoutDescription, LayoutNode, LayoutPane,
        LayoutSetSplitRatioParams, ResponseResult, SplitDirection,
    },
};
use maestro_workspace::layout::PaneId;
use serde_json::json;

/// The layout an export answers with.
fn exported(result: Result<ResponseResult, ErrorBody>) -> LayoutDescription {
    match result {
        Ok(ResponseResult::LayoutExport { layout }) => layout,
        other => panic!("expected an exported layout: {other:?}"),
    }
}

/// The layout a ratio change answers with.
fn adjusted(result: Result<ResponseResult, ErrorBody>) -> LayoutDescription {
    match result {
        Ok(ResponseResult::LayoutSplitRatioSet { layout }) => layout,
        other => panic!("expected the adjusted layout: {other:?}"),
    }
}

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

/// A workspace whose first tab holds one pane and whose second tab splits two.
///
/// Returns the daemon with the panes of the second tab: its root and the one split off it.
fn two_tabs() -> (TestApp, PaneId, PaneId) {
    let mut app = daemon_with_workspaces(&["/one"]);
    let root = add_tab(&mut app, 0);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
    (app, root, right)
}

/// The terminal of `pane` in the first workspace.
fn terminal_of(app: &TestApp, pane: PaneId) -> maestro_workspace::terminal_id::TerminalId {
    app.state.topology.workspaces[0]
        .terminal_id(pane)
        .unwrap()
        .clone()
}

#[test]
fn export_describes_the_focused_tab_as_a_portable_tree() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let root = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
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
        exported(request(&mut app, json!({"type": "layout.export"}))),
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
    let mut app = daemon_with_workspaces(&["/one"]);
    let root = root_pane(&app, 0);
    app.state.topology.workspaces[0].tabs[0].zoomed = true;
    let terminal = terminal_of(&app, root);
    app.state.terminals.get_mut(&terminal).unwrap().launch_argv =
        Some(vec!["just".into(), "dev".into()]);
    let layout = exported(request(&mut app, json!({"type": "layout.export"})));
    assert!(layout.zoomed);
    let LayoutNode::Pane { pane } = layout.root else {
        panic!("a single pane exports as a leaf");
    };
    assert_eq!(pane.command, Some(vec!["just".into(), "dev".into()]));
    assert!(pane.env.is_empty());
}

#[test]
fn export_names_its_tab_directly_or_through_one_of_its_panes() {
    let (mut app, _, _) = two_tabs();
    let default = exported(request(&mut app, json!({"type": "layout.export"})));
    assert_eq!(default.tab_id, "w1:t1");
    for wire in [
        json!({"type": "layout.export", "tab_id": "w1:t2"}),
        json!({"type": "layout.export", "pane_id": "w1:p3"}),
    ] {
        let layout = exported(request(&mut app, wire.clone()));
        assert_eq!(layout.tab_id, "w1:t2", "{wire}");
        assert_eq!(layout.focused_pane_id, "w1:p2", "{wire}");
        assert!(matches!(layout.root, LayoutNode::Split { .. }), "{wire}");
    }
}

#[test]
fn export_refuses_a_target_that_is_missing_or_ambiguous() {
    let (mut app, _, _) = two_tabs();
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
    let mut app = daemon_with_workspaces(&["/one"]);
    let root = root_pane(&app, 0);
    split_pane(&mut app, 0, root, SplitDirection::Right);
    let wire = json!({"type": "layout.set_split_ratio", "path": [], "ratio": 0.72});
    let layout = adjusted(request(&mut app, wire));
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
fn a_split_ratio_change_reaches_a_nested_split_by_its_path() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let root = root_pane(&app, 0);
    let right = split_pane(&mut app, 0, root, SplitDirection::Right);
    split_pane(&mut app, 0, right, SplitDirection::Down);
    let wire = json!({"type": "layout.set_split_ratio", "path": [true], "ratio": 0.3});
    let layout = adjusted(request(&mut app, wire));
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
    let mut app = daemon_with_workspaces(&["/one"]);
    let root = root_pane(&app, 0);
    split_pane(&mut app, 0, root, SplitDirection::Right);
    for (requested, kept) in [(5.0, 0.9), (-1.0, 0.1)] {
        let wire = json!({"type": "layout.set_split_ratio", "path": [], "ratio": requested});
        let LayoutNode::Split { ratio, .. } = adjusted(request(&mut app, wire)).root else {
            panic!("the tab is split");
        };
        assert!((ratio - kept).abs() < f32::EPSILON, "{requested}");
    }
}

#[test]
fn a_split_ratio_for_a_missing_split_is_refused_without_a_change() {
    let mut app = daemon_with_workspaces(&["/one"]);
    for path in [json!([]), json!([true])] {
        let wire = json!({"type": "layout.set_split_ratio", "path": path, "ratio": 0.72});
        assert_eq!(
            request(&mut app, wire),
            failure("split_not_found", "split path not found"),
            "{path}"
        );
    }
    let root = root_pane(&app, 0);
    split_pane(&mut app, 0, root, SplitDirection::Right);
    let wire = json!({"type": "layout.set_split_ratio", "path": [true], "ratio": 0.72});
    assert_eq!(
        request(&mut app, wire),
        failure("split_not_found", "split path not found")
    );
    assert!(!app.state.session_dirty);
    assert!(events_after(&app, 0).is_empty());
}

#[test]
fn a_split_ratio_that_is_not_finite_is_refused() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let root = root_pane(&app, 0);
    split_pane(&mut app, 0, root, SplitDirection::Right);
    for ratio in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let command = RpcCommand::LayoutSetSplitRatio(LayoutSetSplitRatioParams {
            tab_id: None,
            pane_id: None,
            path: Vec::new(),
            ratio,
        });
        let result = block_on(answer_command(&mut app, command));
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
    let (mut app, _, _) = two_tabs();
    for (target, tab) in [
        (json!({"tab_id": "w1:t2"}), "w1:t2"),
        (json!({"pane_id": "w1:p3"}), "w1:t2"),
    ] {
        let mut wire = json!({"type": "layout.set_split_ratio", "path": [], "ratio": 0.4});
        wire.as_object_mut()
            .unwrap()
            .extend(target.as_object().unwrap().clone());
        assert_eq!(adjusted(request(&mut app, wire)).tab_id, tab, "{target}");
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

/// The tab count, terminal count and event count that a refused apply must leave as they were.
fn footprint(app: &TestApp) -> (usize, usize, usize, bool) {
    (
        app.state.topology.workspaces[0].tabs.len(),
        app.state.terminals.len(),
        events_after(app, 0).len(),
        app.state.session_dirty,
    )
}

#[test]
fn applying_to_a_target_that_is_missing_or_ambiguous_is_refused_before_a_process_starts() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let before = footprint(&app);
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
    assert_eq!(footprint(&app), before);
}

#[test]
fn a_leaf_that_cannot_be_launched_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let before = footprint(&app);
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
    assert_eq!(footprint(&app), before);
}

#[test]
fn a_layout_split_that_is_not_finite_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let before = footprint(&app);
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
    assert_eq!(footprint(&app), before);
}

#[test]
fn a_layout_deeper_than_sixteen_levels_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let before = footprint(&app);
    for leaves in [17, 20] {
        let wire = json!({"type": "layout.apply", "root": chain(leaves, &sleeper())});
        assert_eq!(
            request(&mut app, wire),
            failure("invalid_layout", "layout depth is 17; maximum is 16"),
            "{leaves}"
        );
    }
    assert_eq!(footprint(&app), before);
}

#[test]
fn a_layout_of_more_than_twenty_four_panes_is_refused_before_anything_is_created() {
    let mut app = daemon_with_workspaces(&["/one"]);
    let before = footprint(&app);
    let wire = json!({"type": "layout.apply", "root": balanced(25, &sleeper())});
    assert_eq!(
        request(&mut app, wire),
        failure("invalid_layout", "layout has more than 24 panes")
    );
    assert_eq!(footprint(&app), before);
}
