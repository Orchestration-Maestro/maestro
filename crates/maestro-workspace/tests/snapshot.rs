//! Current-format topology and history records.
use super::support::snapshot::{Directory, empty};
use maestro_workspace::persist::{load, save_to_paths};

#[test]
fn empty_snapshot_keeps_sidebar_selection() {
    let dir = Directory::new();
    let path = dir.0.join("session.json");
    save_to_paths(&path, &dir.0.join("history.json"), &empty(), None).unwrap();
    let actual = load(&path).unwrap();
    assert!(actual.workspaces.is_empty());
    assert_eq!(actual.active, None);
    assert_eq!(actual.selected, 0);
    assert_eq!(actual.sidebar_width, Some(26));
    assert_eq!(actual.sidebar_section_split, Some(0.5));
}

#[test]
fn nested_layout_keeps_axes_and_ratios() {
    let expected = serde_json::json!({"Split":{"direction":"Horizontal","ratio":0.25,"first":{"Pane":7},"second":{"Split":{"direction":"Vertical","ratio":0.75,"first":{"Pane":31},"second":{"Pane":99}}}}});
    let layout: maestro_workspace::persist::LayoutSnapshot =
        serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(layout).unwrap(), expected);
}

use super::support::snapshot::{assert_error, capture, encoded, history, populated, read};
use maestro_workspace::persist::{
    load_history,
    snapshot::{self, WorkspaceSnapshot},
};
use serde_json::{Value, json};
use std::fs;

#[test]
fn workspace_snapshot_keeps_names_and_public_numbers() {
    let expected = populated();
    let snapshot = read(&expected).unwrap();
    assert_eq!(encoded(&snapshot), expected);
    let workspace = &snapshot.workspaces[0];
    assert_eq!(workspace.custom_name.as_deref(), Some("Named workspace"));
    assert_eq!(workspace.tabs[0].custom_name.as_deref(), Some("Named tab"));
    assert_eq!(workspace.public_pane_numbers[&9], 47);
    assert_eq!(workspace.public_pane_numbers[&20], 81);
    assert_eq!(workspace.next_public_pane_number, 82);
    assert_eq!(workspace.public_tab_numbers, [17]);
    assert_eq!(workspace.next_public_tab_number, 18);
}

/// Adds the documented serialization defaults to independently recorded inputs.
fn fixture_expected(input: &Value) -> Value {
    let mut expected = input.clone();
    let header = expected.as_object_mut().unwrap();
    header.shift_remove("agent_panel_scope");
    header.entry("sidebar_width").or_insert(Value::Null);
    header.entry("sidebar_section_split").or_insert(Value::Null);
    header.insert("collapsed_space_keys".into(), json!([]));
    for workspace in expected["workspaces"].as_array_mut().unwrap() {
        let fields = workspace.as_object_mut().unwrap();
        for (key, value) in [
            ("public_pane_numbers", json!({})),
            ("next_public_pane_number", json!(0)),
            ("public_tab_numbers", json!([])),
            ("next_public_tab_number", json!(0)),
        ] {
            fields.insert(key.into(), value);
        }
    }
    expected
}

#[test]
fn current_snapshot_fixture_loads_v1() {
    let input: Value =
        serde_json::from_str(include_str!("fixtures/snapshot/current.json")).unwrap();
    let snapshot = read(&input).unwrap();
    assert_eq!(snapshot.version, 1);
    assert_eq!(snapshot.workspaces.len(), 2);
    assert_eq!(snapshot.workspaces[0].tabs.len(), 2);
    assert_eq!(snapshot.active, Some(0));
    assert_eq!(snapshot.selected, 0);
    assert_eq!(snapshot.sidebar_width, None);
    assert_eq!(snapshot.sidebar_section_split, None);
    assert_eq!(
        snapshot.workspaces[1].identity_cwd,
        std::path::Path::new("/home/test/projects/project-b")
    );
    assert_eq!(encoded(&snapshot), fixture_expected(&input));
}

#[test]
fn additive_snapshot_fixture_ignores_unknown_fields() {
    let input: Value =
        serde_json::from_str(include_str!("fixtures/snapshot/additive.json")).unwrap();
    assert_eq!(input["agent_panel_scope"], "CurrentWorkspace");
    let snapshot = read(&input).unwrap();
    assert_eq!(snapshot.sidebar_section_split, Some(0.4));
    assert_eq!(snapshot.workspaces[0].active_tab, 1);
    assert_eq!(snapshot.workspaces[1].tabs[0].panes.len(), 2);
    assert_eq!(encoded(&snapshot), fixture_expected(&input));
}

#[test]
fn newer_snapshot_is_refused() {
    let dir = Directory::new();
    let path = dir.put(&json!({"version":999,"workspaces":[{"tabs":false}]}));
    let (loaded, events) = capture(|| load(&path));
    assert!(loaded.is_none());
    assert_eq!(events[0]["file_version"], "999");
    // A malformed raw header still fails before the future-version branch.
    let (loaded, events) = capture(|| read(&json!({"version":999,"workspaces":false})));
    assert!(loaded.is_none());
    assert_eq!(
        events[0]["message"],
        "failed to parse session file, ignoring"
    );
    assert!(events[0]["err"].contains("expected a sequence"));
}

#[test]
fn workspace_defaults_active_tab_to_zero() {
    let workspace: WorkspaceSnapshot =
        serde_json::from_value(json!({"identity_cwd":"/checkout","tabs":[]})).unwrap();
    assert_eq!(workspace.active_tab, 0);
    assert!(workspace.public_pane_numbers.is_empty());
    assert!(workspace.public_tab_numbers.is_empty());
    assert_eq!(workspace.next_public_pane_number, 0);
    assert_eq!(workspace.next_public_tab_number, 0);
}

#[test]
fn snapshot_versions_require_one() {
    assert!(read(&json!({"version":1})).unwrap().workspaces.is_empty());
    for input in [json!({}), json!({"version":0})] {
        assert_error(&input, "snapshot version 0 is older than supported 1");
    }
    let dir = Directory::new();
    let path = dir.put(&json!({"version":0,"workspaces":[]}));
    let before = fs::read(&path).unwrap();
    assert!(load(&path).is_none());
    assert_eq!(fs::read(path).unwrap(), before);
}

#[test]
fn snapshot_retains_optional_pane_and_worktree_data() {
    let mut value = populated();
    value["collapsed_space_keys"] = json!(["group"]);
    value["workspaces"][0]["worktree_space"] = json!({"key":"group","label":"Group","repo_root":"/repo","checkout_path":"/checkout","is_linked_worktree":true});
    for (id, kind, argv) in [
        ("9", "id", json!(["", "agent", "resume"])),
        ("20", "path", json!([])),
    ] {
        let pane = &mut value["workspaces"][0]["tabs"][0]["panes"][id];
        pane["agent_name"] = json!("Agent");
        pane["managed_agent_kind"] = json!("managed");
        pane["agent_session"] =
            json!({"source":"local","agent":"agent","kind":kind,"value":"reference"});
        pane["launch_argv"] = argv;
    }
    let snapshot = read(&value).unwrap();
    assert!(snapshot.collapsed_space_keys.contains("group"));
    assert_eq!(encoded(&snapshot), value);
    let minimal = read(&populated()).unwrap();
    let encoded = serde_json::to_value(minimal).unwrap();
    let pane = encoded["workspaces"][0]["tabs"][0]["panes"]["20"]
        .as_object()
        .unwrap();
    assert_eq!(pane.len(), 1);
    assert_eq!(pane["cwd"], "/second");
    assert!(encoded["workspaces"][0].get("worktree_space").is_none());
}

#[test]
fn history_snapshot_round_trips_and_checks_versions() {
    let dir = Directory::new();
    let path = dir.0.join("history.json");
    let expected = serde_json::to_value(history()).unwrap();
    fs::write(&path, serde_json::to_vec_pretty(&expected).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(load_history(&path).unwrap()).unwrap(),
        expected
    );
    for (version, text) in [(0, "older"), (999, "newer")] {
        let mut input = expected.clone();
        input["version"] = json!(version);
        let path = dir.put(&input);
        let (loaded, events) = capture(|| load_history(&path));
        assert!(loaded.is_none());
        if version == 0 {
            assert_eq!(
                events[0]["err"],
                format!("history snapshot version {version} is {text} than supported 1")
            );
        } else {
            assert_eq!(events[0]["file_version"], "999");
        }
    }
    for invalid in [
        json!({"workspaces":[]}),
        json!({"version":1}),
        json!({"version":1,"workspaces":[{"tabs":[{"panes":{"1":{"ansi":null,"lines":0}}}]}]}),
    ] {
        assert!(load_history(&dir.put(&invalid)).is_none());
    }
}

#[test]
fn capture_node_preserves_both_split_axes() {
    use maestro_protocol::schema::SplitDirection;
    use maestro_workspace::layout::{Node, PaneId};
    let node = Node::Split {
        direction: SplitDirection::Right,
        ratio: 0.25,
        first: Box::new(Node::Pane(PaneId::from_raw(9))),
        second: Box::new(Node::Split {
            direction: SplitDirection::Down,
            ratio: 0.75,
            first: Box::new(Node::Pane(PaneId::from_raw(31))),
            second: Box::new(Node::Pane(PaneId::from_raw(99))),
        }),
    };
    assert_eq!(
        serde_json::to_value(snapshot::capture_node(&node)).unwrap(),
        json!({"Split":{"direction":"Horizontal","ratio":0.25,"first":{"Pane":9},"second":{"Split":{"direction":"Vertical","ratio":0.75,"first":{"Pane":31},"second":{"Pane":99}}}}})
    );
}

/// Rejects invalid values at a reached raw-header or typed-workspace field.
fn rejects(pointer: &str, values: &[Value]) {
    for invalid in values {
        let mut input = populated();
        *input.pointer_mut(pointer).unwrap() = invalid.clone();
        assert!(read(&input).is_none(), "invalid field at {pointer}");
    }
}

/// Required nested fields fail when absent as well as when null.
fn rejects_missing(pointer: &str, fields: &[&str]) {
    for field in fields {
        let mut input = populated();
        input
            .pointer_mut(pointer)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .shift_remove(*field);
        assert!(read(&input).is_none(), "missing field {field}");
    }
}

#[test]
fn snapshot_field_defaults_and_type_errors() {
    assert_header_defaults();
    for pointer in [
        "/version",
        "/selected",
        "/active",
        "/sidebar_width",
        "/workspaces/0/active_tab",
        "/workspaces/0/next_public_pane_number",
        "/workspaces/0/next_public_tab_number",
        "/workspaces/0/tabs/0/focused",
        "/workspaces/0/tabs/0/root_pane",
    ] {
        rejects(
            pointer,
            &[json!(-1), json!(1.5), json!("1"), json!({}), json!([])],
        );
    }
    rejects("/sidebar_width", &[json!(65536)]);
    for pointer in [
        "/version",
        "/selected",
        "/workspaces",
        "/collapsed_space_keys",
        "/workspaces/0/identity_cwd",
        "/workspaces/0/public_pane_numbers",
        "/workspaces/0/public_tab_numbers",
        "/workspaces/0/tabs",
        "/workspaces/0/tabs/0/panes",
        "/workspaces/0/tabs/0/zoomed",
        "/workspaces/0/tabs/0/panes/9/cwd",
    ] {
        rejects(pointer, &[Value::Null]);
    }
    for pointer in [
        "/workspaces/0/custom_name",
        "/workspaces/0/id",
        "/workspaces/0/tabs/0/custom_name",
        "/workspaces/0/tabs/0/panes/9/label",
    ] {
        rejects(pointer, &[json!(3), json!([]), json!({})]);
    }
    rejects("/collapsed_space_keys", &[json!([1])]);
    rejects(
        "/workspaces/0/public_pane_numbers",
        &[json!({"-1":2}), json!({"4294967296":2}), json!({"9":-1})],
    );
    rejects(
        "/workspaces/0/public_tab_numbers",
        &[json!([-1]), json!([0.5])],
    );
    rejects_missing("/workspaces/0", &["identity_cwd", "tabs"]);
    rejects_missing("/workspaces/0/tabs/0", &["layout", "panes", "zoomed"]);
    rejects_missing("/workspaces/0/tabs/0/panes/9", &["cwd"]);
    assert_optional_and_variant_domains();
    assert_positional_and_unknown_records();
    assert_malformed_and_duplicate_fields();
}

fn assert_optional_and_variant_domains() {
    let mut value = populated();
    for key in [
        "label",
        "agent_name",
        "managed_agent_kind",
        "agent_session",
        "launch_argv",
    ] {
        value["workspaces"][0]["tabs"][0]["panes"]["9"][key] = Value::Null;
    }
    value["workspaces"][0]["worktree_space"] = Value::Null;
    value["sidebar_width"] = Value::Null;
    value["sidebar_section_split"] = Value::Null;
    assert!(read(&value).is_some());
    for (key, invalid) in [
        ("agent_name", json!(1)),
        ("managed_agent_kind", json!([])),
        (
            "agent_session",
            json!({"source":"s","agent":"a","kind":"unknown","value":"v"}),
        ),
        ("agent_session", json!({})),
        ("launch_argv", json!("cmd")),
        ("launch_argv", json!([3])),
    ] {
        value["workspaces"][0]["tabs"][0]["panes"]["9"][key] = invalid;
        assert!(read(&value).is_none());
        value["workspaces"][0]["tabs"][0]["panes"]["9"][key] = Value::Null;
    }
    value["workspaces"][0]["worktree_space"] = json!({"key":"group"});
    assert!(read(&value).is_none());
    rejects(
        "/workspaces/0/tabs/0/layout",
        &[
            json!({"Unknown":9}),
            json!({"Pane":-1}),
            json!({"Split":{}}),
        ],
    );
    rejects(
        "/workspaces/0/tabs/0/layout/Split/direction",
        &[json!("Right"), Value::Null],
    );
    rejects(
        "/workspaces/0/tabs/0/layout/Split/ratio",
        &[json!("0.5"), Value::Null],
    );
    for ratio in [0.0, -2.0, 3.0] {
        let mut input = populated();
        input["workspaces"][0]["tabs"][0]["layout"]["Split"]["ratio"] = json!(ratio);
        assert!(read(&input).is_some());
    }
}

fn assert_positional_and_unknown_records() {
    let positional = read(&json!([1, [], null, 3, null, null, []])).unwrap();
    assert_eq!(positional.selected, 3);
    let mut input = populated();
    input["workspaces"][0]["tabs"][0]["panes"]["20"] =
        json!(["/second", null, null, null, null, []]);
    assert_eq!(
        read(&input).unwrap().workspaces[0].tabs[0].panes[&20].launch_argv,
        Some(vec![])
    );
    input["extra"] = json!({"ignored":true});
    input["workspaces"][0]["tabs"][0]["panes"]["9"]["history"] =
        json!({"ansi":"not topology","lines":8});
    let encoded = serde_json::to_value(read(&input).unwrap()).unwrap();
    assert!(encoded.get("extra").is_none());
    assert!(
        encoded["workspaces"][0]["tabs"][0]["panes"]["9"]
            .get("history")
            .is_none()
    );
}

/// Confirms absent raw-header options without a typed workspace.
fn assert_header_defaults() {
    let defaults = read(&json!({"version":1})).unwrap();
    assert!(defaults.workspaces.is_empty());
    assert_eq!(defaults.active, None);
    assert_eq!(defaults.selected, 0);
    assert_eq!(defaults.sidebar_width, None);
    assert_eq!(defaults.sidebar_section_split, None);
    assert!(defaults.collapsed_space_keys.is_empty());
}

/// Retains native JSON framing and duplicate-known-field rejection.
fn assert_malformed_and_duplicate_fields() {
    let dir = Directory::new();
    for bytes in [
        "{",
        r#"{"version":1,"version":1}"#,
        r#"{"version":1,"selected":0,"selected":1}"#,
    ] {
        let path = dir.0.join("malformed.json");
        fs::write(&path, bytes).unwrap();
        assert!(load(&path).is_none());
    }
}
