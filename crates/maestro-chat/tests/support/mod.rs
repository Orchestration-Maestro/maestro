//! Strict recorded inputs replayed through the public navigation interface.
pub mod registry;
pub mod tree;
use maestro_app::{SessionTreeNode, presentation_data::keybindings::KEYBINDINGS};
use maestro_chat::components::tree_selector::{FilterMode, TreeList};
use maestro_tui::{
    KeybindingKeys, KeybindingsConfig, KeybindingsManager, get_keybindings, set_keybindings,
};
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
/// Accepted caller binding shapes.
#[derive(Deserialize)]
#[serde(untagged)]
enum Binding {
    /// One key.
    Single(String),
    /// An explicit list, including a disabled empty list.
    Multiple(Vec<String>),
}
/// A key event or an active manager change.
#[derive(Deserialize)]
#[serde(untagged)]
enum Operation {
    /// Raw protocol bytes.
    Key(String),
    /// Manager replacement or live configuration change.
    Registry(RegistryOperation),
}
/// The two recorded registry operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RegistryOperation {
    /// Replacement manager's overrides.
    replace_bindings: Option<BTreeMap<String, Binding>>,
    /// Live manager's replacement overrides.
    set_bindings: Option<BTreeMap<String, Binding>>,
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
/// Converts only the accepted recorded filter vocabulary.
fn mode(value: &str) -> FilterMode {
    match value {
        "default" => FilterMode::Default,
        "no-tools" => FilterMode::NoTools,
        "user-only" => FilterMode::UserOnly,
        "labeled-only" => FilterMode::LabeledOnly,
        "all" => FilterMode::All,
        _ => panic!("unrecognized fixture mode"),
    }
}
/// Uses the toolkit's original scalar/list override shapes.
fn bindings(values: &BTreeMap<String, Binding>) -> KeybindingsConfig {
    values
        .iter()
        .map(|(id, value)| {
            let keys = match value {
                Binding::Single(key) => KeybindingKeys::Single(key.clone()),
                Binding::Multiple(keys) => KeybindingKeys::Multiple(keys.clone()),
            };
            (id.clone(), Some(keys))
        })
        .collect()
}
/// Resolves every supplied occurrence to its original node address.
fn occurrences<'tree, 'entry>(
    tree: &'tree [SessionTreeNode<'entry>],
    input: &'entry Input,
) -> Vec<&'tree SessionTreeNode<'entry>> {
    let mut nodes = vec![None; input.nodes.len()];
    let mut pending: Vec<_> = tree.iter().collect();
    while let Some(node) = pending.pop() {
        let index = input
            .nodes
            .iter()
            .position(|value| std::ptr::eq(&raw const value.entry, node.entry))
            .unwrap();
        nodes[index] = Some(node);
        pending.extend(&node.children);
    }
    nodes.into_iter().map(Option::unwrap).collect()
}
/// Checks occurrence identity and a full ordered visible cycle through public keys.
fn frame(
    list: &mut TreeList<'_, '_>,
    expected: &Frame,
    nodes: &[&SessionTreeNode<'_>],
    events: &RefCell<Vec<Vec<String>>>,
) {
    assert_eq!(*events.borrow(), expected.events, "callback transcript");
    let selected = list.get_selected_node();
    assert_eq!(
        selected.map(|node| nodes
            .iter()
            .position(|value| std::ptr::eq(*value, node))
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
                std::ptr::eq(list.get_selected_node().unwrap(), nodes[occurrence]),
                "visible occurrence order"
            );
            list.handle_input("\u{1b}[B").unwrap();
        }
        assert!(
            std::ptr::eq(list.get_selected_node().unwrap(), nodes[index]),
            "full cycle wraps to original occurrence"
        );
    } else {
        assert!(expected.visible.is_empty());
    }
}
/// Drives a key event or caller configuration change through the existing manager.
fn apply_operation(list: &mut TreeList<'_, '_>, operation: &Operation) {
    match operation {
        Operation::Key(data) => list.handle_input(data).unwrap(),
        Operation::Registry(change) => {
            assert_ne!(
                change.replace_bindings.is_some(),
                change.set_bindings.is_some()
            );
            if let Some(values) = &change.replace_bindings {
                set_keybindings(KeybindingsManager::new(
                    KEYBINDINGS.clone(),
                    bindings(values),
                ));
            }
            if let Some(values) = &change.set_bindings {
                get_keybindings().set_user_bindings(bindings(values));
            }
        }
    }
}

/// Replays one scenario, preserving actual callback order and occurrence identity.
fn replay(case: &Case) {
    set_keybindings(KeybindingsManager::new(
        KEYBINDINGS.clone(),
        bindings(&case.input.bindings),
    ));
    let tree = tree(&case.input.nodes, &case.input.roots);
    let nodes = occurrences(&tree, &case.input);
    let result = TreeList::new(
        &tree,
        case.input.leaf.as_deref(),
        case.input.lines,
        case.input.initial.as_deref(),
        case.input.mode.as_deref().map(mode),
    );
    let mut list = match result {
        Ok(list) => {
            assert!(
                case.expected.error.is_none(),
                "expected constructor failure"
            );
            list
        }
        Err(error) => {
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
            assert_eq!(case.expected.error.as_ref().unwrap().kind, "InvalidData");
            assert!(case.expected.frames.is_empty());
            return;
        }
    };
    let events = Rc::new(RefCell::new(Vec::new()));
    if case.input.callbacks {
        let selected = events.clone();
        list.on_select = Some(Box::new(move |id| {
            selected.borrow_mut().push(vec!["select".into(), id.into()]);
        }));
        let canceled = events.clone();
        list.on_cancel = Some(Box::new(move || {
            canceled.borrow_mut().push(vec!["cancel".into()]);
        }));
    }
    assert_eq!(case.expected.frames.len(), case.input.ops.len() + 1);
    frame(&mut list, &case.expected.frames[0], &nodes, &events);
    for (operation, expected) in case.input.ops.iter().zip(&case.expected.frames[1..]) {
        apply_operation(&mut list, operation);
        frame(&mut list, expected, &nodes, &events);
    }
    if case.name.starts_with("opaque_entry_fields") {
        let entry = list.get_selected_node().unwrap().entry;
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
