//! Shared ordering through the application's public session-tree route.
#![cfg(test)]
use crate::tree;
use maestro_app::{SessionTreeNode as InputNode, export_html::template};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashSet;

/// Strict supplied oracle envelope.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Complete geometry inputs and outputs.
    cases: Vec<Case>,
    /// Independent raw slot inputs.
    metadata: Vec<Metadata>,
    /// Reached message receiver classes.
    receivers: Vec<Receiver>,
}
/// One complete oracle invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Diagnostic name.
    id: String,
    /// Owning public witness.
    test: String,
    /// Borrowed tree operands.
    input: Input,
    /// Full output records.
    expected: Expected,
}
/// Source records and physical occurrence links.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Raw immutable records.
    entries: Vec<Value>,
    /// Child topology and independent metadata.
    occurrences: Vec<Occurrence>,
    /// Supplied root positions.
    roots: Vec<usize>,
    /// Active identity.
    leaf: Option<String>,
    /// Visible occurrence set, or every occurrence.
    visible: Option<Vec<usize>>,
}
/// One occurrence referencing a source entry.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Occurrence {
    /// Source entry index.
    entry: usize,
    /// Ordered physical child positions.
    children: Vec<usize>,
    /// Original occurrence fields for the existing optional-slot decoder.
    #[serde(default, deserialize_with = "tree::present_value")]
    label: Option<Value>,
}
/// Expected complete geometry observation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Physical occurrence order.
    ordered: Vec<usize>,
    /// Physical rows in order.
    physical: Vec<Row>,
    /// Ordered visible occurrences.
    visible: Vec<usize>,
    /// Visible row geometry.
    projected: Vec<Row>,
    /// Last-ID parent lookup.
    parents: Vec<(String, Option<String>)>,
    /// Ordered children by parent identity.
    children: Vec<(Option<String>, Vec<String>)>,
    /// Virtual-root flag.
    multiple_roots: bool,
}
/// Exact row fields from the independent oracle.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Row {
    /// Logical indentation.
    indent: usize,
    /// Branch connector flag.
    show_connector: bool,
    /// Last sibling flag.
    is_last: bool,
    /// Ordered continuation gutters.
    gutters: Vec<Gutter>,
    /// Virtual-root child flag.
    virtual_child: bool,
}
/// One continuing gutter.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Gutter {
    /// Original level.
    position: usize,
    /// Continuation visibility.
    show: bool,
}
/// Raw label and timestamp probe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Metadata {
    /// Class name.
    id: String,
    /// Optional fields whose presence is significant.
    input: RawSlots,
    /// Independent presence and value observations.
    expected: MetadataExpected,
}
/// Optional raw slots with strict field recognition.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSlots {
    /// Label presence and raw value.
    #[serde(default, deserialize_with = "tree::present_value")]
    label: Option<Value>,
    /// Timestamp presence and raw value.
    #[serde(default, deserialize_with = "tree::present_value")]
    label_timestamp: Option<Value>,
}
/// Expected independent slot observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataExpected {
    /// Label presence.
    label_present: bool,
    /// Timestamp presence.
    label_timestamp_present: bool,
    /// Expected raw label.
    #[serde(default, deserialize_with = "tree::present_value")]
    label: Option<Value>,
    /// Expected raw timestamp.
    #[serde(default, deserialize_with = "tree::present_value")]
    label_timestamp: Option<Value>,
}
/// Message receiver probe.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Receiver {
    /// Class name.
    id: String,
    /// Original entry.
    entry: Value,
    /// Success or invalid-data outcome.
    result: String,
}
/// Loads the complete corpus and rejects repeated complete operands.
fn corpus() -> Corpus {
    let raw: Value = serde_json::from_str(include_str!("fixtures/tree-projection.json")).unwrap();
    for (field, operand) in [
        ("cases", "input"),
        ("metadata", "input"),
        ("receivers", "entry"),
    ] {
        let mut seen = HashSet::new();
        for case in raw[field].as_array().unwrap() {
            assert!(
                seen.insert(case[operand].to_string()),
                "duplicate {field} input"
            );
        }
    }
    serde_json::from_value(raw).unwrap()
}
/// Stable ordering and raw metadata survive the shared retention boundary.
#[test]
fn shared_tree_order_keeps_stable_occurrences() {
    check_ordering_cases("shared_tree_order_keeps_stable_occurrences");
    for case in corpus().metadata {
        let entry =
            serde_json::json!({"id":"a", "type":"message", "message":{}, "opaque":{"kept":true}});
        let node = InputNode {
            entry: &entry,
            children: vec![],
            label: case.input.label.as_ref(),
            label_timestamp: case.input.label_timestamp.as_ref(),
        };
        let roots = [node];
        let ordered = template::flatten_tree(&roots, None).unwrap();
        let retained = template::retain(&ordered);
        assert_eq!(**retained[0].entry(), entry, "{}", case.id);
        assert_eq!(
            retained[0].label().is_some(),
            case.expected.label_present,
            "{}",
            case.id
        );
        assert_eq!(*retained[0].label(), case.expected.label, "{}", case.id);
        assert_eq!(
            retained[0].label_timestamp().is_some(),
            case.expected.label_timestamp_present,
            "{}",
            case.id
        );
        assert_eq!(
            *retained[0].label_timestamp(),
            case.expected.label_timestamp,
            "{}",
            case.id
        );
    }
}

/// Builds ordinary trees through the existing helper and associates physical positions.
fn check_ordering_cases(test: &str) {
    for case in corpus().cases.into_iter().filter(|case| case.test == test) {
        let nodes = input_nodes(&case.input);
        let roots = input_roots(&case, &nodes);
        let mut positions = std::collections::HashMap::new();
        let mut pending: Vec<_> = case.input.roots.iter().copied().zip(&roots).collect();
        while let Some((index, node)) = pending.pop() {
            positions.insert(std::ptr::from_ref(node), index);
            pending.extend(
                case.input.occurrences[index]
                    .children
                    .iter()
                    .copied()
                    .zip(&node.children),
            );
        }
        let ordered = template::flatten_tree(&roots, case.input.leaf.as_deref()).unwrap();
        let observed: Vec<_> = ordered
            .iter()
            .map(|node| positions[&std::ptr::from_ref(*node)])
            .collect();
        assert_eq!(observed, case.expected.ordered, "{}", case.id);
        let retained = template::retain(&ordered);
        for (node, &index) in retained.iter().zip(&observed) {
            let occurrence = &case.input.occurrences[index];
            assert_eq!(
                **node.entry(),
                case.input.entries[occurrence.entry],
                "{}",
                case.id
            );
            assert_eq!(*node.label(), occurrence.label, "{}", case.id);
        }
        if case.id == "shared-entry-distinct-occurrences" {
            assert!(std::rc::Rc::ptr_eq(
                retained[1].entry(),
                retained[2].entry()
            ));
            assert!(retained[1] != retained[2]);
        }
        drop(roots);
        check_projection(case, &retained, &observed);
    }
}

/// Complete physical branch and forest geometry matches the independent rows.
#[test]
fn shared_tree_geometry_preserves_branch_gutters() {
    check_ordering_cases("shared_tree_geometry_preserves_branch_gutters");
}

/// Compares every row and ordered gutter field without reducing occurrences to IDs.
fn check_rows(actual: &[template::FlatNode], expected: &[Row]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_eq!(actual.indent, expected.indent);
        assert_eq!(actual.show_connector, expected.show_connector);
        assert_eq!(actual.is_last, expected.is_last);
        assert_eq!(actual.virtual_child, expected.virtual_child);
        assert_eq!(actual.gutters.len(), expected.gutters.len());
        for (actual, expected) in actual.gutters.iter().zip(&expected.gutters) {
            assert_eq!(actual.position, expected.position);
            assert_eq!(actual.show, expected.show);
        }
    }
}

/// Hidden ancestry and repeated IDs retain the complete visible projection.
#[test]
fn shared_tree_projection_uses_visible_ancestry() {
    check_ordering_cases("shared_tree_projection_uses_visible_ancestry");
}

/// Only reached message entries require non-null receivers; other JSON classes pass intact.
#[test]
fn shared_ordering_reads_only_reached_message_receivers() {
    for case in corpus().receivers {
        let roots = [InputNode {
            entry: &case.entry,
            children: vec![],
            label: None,
            label_timestamp: None,
        }];
        let read = template::read_message(&case.entry);
        let ordered = template::flatten_tree(&roots, None);
        match case.result.as_str() {
            "invalid-data" => {
                for error in [read.err().unwrap(), ordered.err().unwrap()] {
                    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData, "{}", case.id);
                    assert_eq!(error.to_string(), "missing message receiver", "{}", case.id);
                }
            }
            "success" => {
                assert!(
                    std::ptr::eq(read.unwrap(), &raw const case.entry["message"]),
                    "{}",
                    case.id
                );
                let ordered = ordered.unwrap();
                assert_eq!(ordered.len(), 1, "{}", case.id);
                assert!(std::ptr::eq(ordered[0], &raw const roots[0]), "{}", case.id);
            }
            _ => panic!("unknown receiver outcome"),
        }
    }
    let nonmessage = serde_json::json!({"id":"ignored", "type":"custom", "message":null});
    let roots = [InputNode {
        entry: &nonmessage,
        children: vec![],
        label: None,
        label_timestamp: None,
    }];
    assert_eq!(template::flatten_tree(&roots, None).unwrap().len(), 1);
}

/// Converts occurrence operands for the existing borrowed tree helper.
fn input_nodes(input: &Input) -> Vec<tree::Node> {
    input
        .occurrences
        .iter()
        .map(|occurrence| tree::Node {
            entry: input.entries[occurrence.entry].clone(),
            children: occurrence.children.clone(),
            label: occurrence.label.clone(),
            label_timestamp: None,
        })
        .collect()
}
/// Uses explicit shared-entry operands only for the identity probe.
fn input_roots<'a>(case: &'a Case, nodes: &'a [tree::Node]) -> Vec<InputNode<'a>> {
    if case.id == "shared-entry-distinct-occurrences" {
        vec![InputNode {
            entry: &case.input.entries[0],
            children: [1, 2]
                .into_iter()
                .map(|index| InputNode {
                    entry: &case.input.entries[1],
                    children: vec![],
                    label: case.input.occurrences[index].label.as_ref(),
                    label_timestamp: None,
                })
                .collect(),
            label: None,
            label_timestamp: None,
        }]
    } else {
        tree::tree(nodes, &case.input.roots)
    }
}
/// Checks all physical and visible observations for a single invocation.
fn check_projection(case: Case, retained: &[template::SessionTreeNode], observed: &[usize]) {
    let physical = template::physical(retained);
    check_rows(&physical, &case.expected.physical);
    let visible: Vec<_> = retained
        .iter()
        .zip(observed)
        .filter(|(_, index)| {
            case.input
                .visible
                .as_ref()
                .is_none_or(|ids| ids.contains(index))
        })
        .map(|(node, _)| node.clone())
        .collect();
    let visible_positions: Vec<_> = retained
        .iter()
        .zip(observed)
        .filter(|(node, _)| visible.contains(node))
        .map(|(_, index)| *index)
        .collect();
    assert_eq!(visible_positions, case.expected.visible, "{}", case.id);
    let parents = retained
        .iter()
        .filter_map(|node| {
            node.entry()["id"]
                .as_str()
                .map(|id| (id.to_owned(), node.clone()))
        })
        .collect::<std::collections::HashMap<_, _>>();
    let projected = template::recalculate_visual_structure(retained, &visible, &physical, &parents);
    check_rows(&projected.rows, &case.expected.projected);
    assert_eq!(
        projected.parents,
        case.expected.parents.into_iter().collect(),
        "{}",
        case.id
    );
    // Missing children and an empty child vector both denote zero children.
    let actual_children: std::collections::HashMap<_, _> = projected
        .children
        .into_iter()
        .filter(|(_, children)| !children.is_empty())
        .collect();
    let expected_children = case
        .expected
        .children
        .into_iter()
        .filter(|(_, children)| !children.is_empty())
        .collect();
    assert_eq!(actual_children, expected_children, "{}", case.id);
    assert_eq!(
        projected.multiple_roots, case.expected.multiple_roots,
        "{}",
        case.id
    );
}
