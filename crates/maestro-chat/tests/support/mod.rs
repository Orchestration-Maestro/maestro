//! Strict recorded inputs replayed through the public navigation interface.
use crate::bindings;
use crate::filter_mode;
pub(crate) use crate::occurrences;
use crate::recorded_registry;
pub(crate) use crate::registry;
pub(crate) use crate::tree;
use bindings::{Binding, bindings};
use filter_mode::mode;
use maestro_app::presentation_data::bindings::KEYBINDINGS;
use maestro_chat::components::tree_picker::{SessionTreeNode, TreeList};
use maestro_tui::{KeybindingsManager, get_keybindings, set_keybindings};
use recorded_registry::{Operation, apply_operation};
use registry::{REGISTRY, Restore};
use serde::Deserialize;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    rc::Rc,
};
use tree::{Node, tree};

/// Recorded scenario inventory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    /// Exact scenario names attributed to each public test.
    groups: BTreeMap<String, Vec<String>>,
    /// Input and expected output for each scenario.
    cases: Vec<Case>,
}
/// One independently recorded invocation sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Distinguishable scenario label.
    #[serde(rename = "case")]
    name: String,
    /// Operands and subsequent actions.
    input: Input,
    /// Ordered observations and any reached failure.
    expected: Expected,
}
/// Public constructor operands and input actions.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    /// Raw retained entries with explicit child occurrence indices.
    nodes: Vec<Node>,
    /// Root occurrences in supplied order.
    roots: Vec<usize>,
    /// Active history identity.
    leaf: Option<String>,
    /// Page distance.
    lines: usize,
    /// Subsequent key or registry operations.
    ops: Vec<Operation>,
    /// Explicit selection when supplied.
    initial: Option<String>,
    /// Initial filter when supplied.
    mode: Option<String>,
    /// Initial caller overrides.
    #[serde(default)]
    bindings: BTreeMap<String, Binding>,
    /// Whether callback slots are installed.
    #[serde(default = "callbacks_enabled")]
    callbacks: bool,
}
/// Defaults recorded invocations to installed callbacks.
fn callbacks_enabled() -> bool {
    true
}
/// Ordered observations from the reference entry point.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    /// Successful frames preceding any failure.
    frames: Vec<Frame>,
    /// Failure kind when reached.
    error: Option<Failure>,
}
/// Recorded invalid-data category, without engine error text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    /// Native error category.
    kind: String,
}
/// A complete state observation after one operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    /// Visible occurrence indices in navigation order.
    visible: Vec<usize>,
    /// Selected occurrence or empty view.
    selected: Option<usize>,
    /// Ordered cumulative callback events.
    events: Vec<Vec<String>>,
}
/// Checks occurrence identity and a full ordered visible cycle through public keys.
fn frame(
    list: &mut TreeList,
    expected: &Frame,
    nodes: &[Option<SessionTreeNode>],
    events: &RefCell<Vec<Vec<String>>>,
) {
    assert_eq!(*events.borrow(), expected.events, "callback transcript");
    let selected = list.get_selected_node();
    assert_eq!(
        selected.as_ref().map(|node| nodes
            .iter()
            .position(|value| value.as_ref() == Some(node))
            .unwrap()),
        expected.selected,
        "selected occurrence"
    );
    if let Some(index) = expected.selected {
        let start = expected
            .visible
            .iter()
            .position(|&item| item == index)
            .unwrap();
        for offset in 0..expected.visible.len() {
            let occurrence = expected.visible[(start + offset) % expected.visible.len()];
            assert!(
                list.get_selected_node().as_ref() == nodes[occurrence].as_ref(),
                "visible occurrence order"
            );
            list.handle_input("\u{1b}[B").unwrap();
        }
        assert!(
            list.get_selected_node().as_ref() == nodes[index].as_ref(),
            "full cycle wraps to original occurrence"
        );
    } else {
        assert!(expected.visible.is_empty());
    }
}
/// Replays one scenario, preserving actual callback order and occurrence identity.
fn replay(case: &Case) {
    set_keybindings(KeybindingsManager::new(
        KEYBINDINGS.clone(),
        bindings(&case.input.bindings),
    ));
    let tree = tree(&case.input.nodes, &case.input.roots);
    let result = TreeList::new(
        &tree,
        case.input.leaf.as_deref(),
        case.input.lines,
        case.input.initial.as_deref(),
        case.input.mode.as_deref().map(mode),
    );
    let mut list = match result {
        Ok(list) => {
            assert!(case.expected.error.is_none(), "constructor failure");
            list
        }
        Err(error) => {
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
            assert_eq!(case.expected.error.as_ref().unwrap().kind, "InvalidData");
            assert!(case.expected.frames.is_empty());
            return;
        }
    };
    let nodes = occurrences::occurrences(&list, &case.input.nodes);
    let events = Rc::new(RefCell::new(Vec::new()));
    if case.input.callbacks {
        let selected = events.clone();
        list.set_on_select(Some(Rc::new(move |id| {
            selected.borrow_mut().push(vec!["select".into(), id.into()]);
        })));
        let canceled = events.clone();
        list.set_on_cancel(Some(Rc::new(move || {
            canceled.borrow_mut().push(vec!["cancel".into()]);
        })));
    }
    assert_eq!(case.expected.frames.len(), case.input.ops.len() + 1);
    frame(&mut list, &case.expected.frames[0], &nodes, &events);
    for (operation, expected) in case.input.ops.iter().zip(&case.expected.frames[1..]) {
        apply_operation(&list, operation);
        let nodes = occurrences::occurrences(&list, &case.input.nodes);
        frame(&mut list, expected, &nodes, &events);
    }
    if case.name.starts_with("opaque_entry_fields") {
        let node = list.get_selected_node().unwrap();
        let entry = node.entry();
        assert_eq!(entry["extension"]["nested"][0]["untouched"], true);
        assert_eq!(entry["extension"]["ordered"]["z"], 1);
        assert_eq!(entry["extension"]["ordered"]["a"], 2);
        assert_eq!(
            entry["extension"]["ordered"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
    }
}
/// Runs precisely the attributed cases, rejecting duplicates and orphan scenarios.
pub fn group(name: &str) {
    let _serial = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = Restore(get_keybindings());
    let corpus: Corpus =
        serde_json::from_str(include_str!("../fixtures/tree-selection.json")).unwrap();
    let attributed: Vec<_> = corpus.groups.values().flatten().collect();
    assert_eq!(attributed.len(), corpus.cases.len());
    assert_eq!(
        attributed.iter().copied().collect::<HashSet<_>>().len(),
        attributed.len()
    );
    let cases: HashSet<_> = corpus.cases.iter().map(|case| &case.name).collect();
    assert_eq!(cases.len(), corpus.cases.len());
    assert_eq!(cases, attributed.into_iter().collect());
    let requested = &corpus.groups[name];
    assert!(!requested.is_empty());
    for id in requested {
        let case = corpus.cases.iter().find(|case| &case.name == id).unwrap();
        eprintln!("scenario: {id}");
        replay(case);
    }
}
