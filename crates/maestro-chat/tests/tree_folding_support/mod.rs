//! Strict search, fold, query, callback and whole-frame corpus replay.
use crate::bindings;
use crate::filter_mode;
use crate::manual_runtime;
use crate::occurrences;
use crate::palette;
use crate::published;
use crate::recorded_registry;
use crate::registry;
use crate::tree;
use bindings::{Binding, bindings};
use filter_mode::mode;
use maestro_app::{compact_json, presentation_data::keybindings::KEYBINDINGS};
use maestro_chat::TreeList;
use maestro_tui::{KeybindingKeys, KeybindingsManager, get_keybindings, set_keybindings};
use recorded_registry::{Operation, apply_operation};
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashSet},
    rc::Rc,
};
use tree::{Node, tree};

/// One source-recorded complete input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    #[serde(rename = "case")]
    name: String,
    input: Input,
    expected: Expected,
}
/// Construction, key configuration and render operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    nodes: Vec<Node>,
    roots: Vec<usize>,
    leaf: Option<String>,
    initial: Option<String>,
    mode: String,
    ops: Vec<Operation>,
    bindings: BTreeMap<String, Binding>,
    widths: Vec<usize>,
    palettes: Vec<String>,
    lines: usize,
    callbacks: bool,
}
/// Ordered expected public observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    frames: Vec<Frame>,
}
/// Complete navigation, query and notification state.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    selected: Option<usize>,
    visible: Vec<usize>,
    query: String,
    events: Vec<Vec<Value>>,
    rendered: Vec<Rendered>,
}
/// Complete styled output at an explicit width and palette.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rendered {
    palette: String,
    width: usize,
    lines: Vec<String>,
}
/// Observes all expected fields, including occurrence order rather than just IDs.
fn frame(list: &TreeList, input: &Input, expected: &Frame, events: &RefCell<Vec<Vec<Value>>>) {
    assert_eq!(list.get_search_query(), expected.query);
    assert_eq!(*events.borrow(), expected.events);
    let host = manual_runtime::ManualRuntime::new();
    let (state, live) = published::published(&palette::palette(&input.palettes[0]));
    assert_eq!(
        expected.rendered.len(),
        input.palettes.len() * input.widths.len()
    );
    for (record, (palette, width)) in expected.rendered.iter().zip(
        input
            .palettes
            .iter()
            .flat_map(|p| input.widths.iter().map(move |w| (p, w))),
    ) {
        assert_eq!((&record.palette, record.width), (palette, *width));
        state
            .set_theme_instance(self::palette::palette(palette))
            .unwrap();
        assert_eq!(
            list.render(*width, &live, &host, true).unwrap(),
            record.lines
        );
    }
    let handles = occurrences::occurrences(list, &input.nodes);
    assert!(expected.selected.is_some() || expected.visible.is_empty());
    assert!(
        list.get_selected_node()
            == expected
                .selected
                .map(|index| handles[index].clone().unwrap())
    );
    let restore = registry::Restore(get_keybindings());
    set_keybindings(KeybindingsManager::new(KEYBINDINGS.clone(), vec![]));
    if let Some(index) = expected.selected {
        let start = expected.visible.iter().position(|&i| i == index).unwrap();
        for offset in 0..expected.visible.len() {
            let occurrence = expected.visible[(start + offset) % expected.visible.len()];
            assert!(list.get_selected_node() == handles[occurrence]);
            list.handle_input("\u{1b}[B").unwrap();
        }
        assert!(list.get_selected_node() == handles[index]);
    }
    drop(restore);
}
/// Replays all frames after construction and every subsequent operation.
fn replay(case: &Case) {
    set_keybindings(KeybindingsManager::new(
        KEYBINDINGS.clone(),
        bindings(&case.input.bindings),
    ));
    let input = &case.input;
    let supplied = tree(&input.nodes, &input.roots);
    let list = TreeList::new(
        &supplied,
        input.leaf.as_deref(),
        input.lines,
        input.initial.as_deref(),
        Some(mode(&input.mode)),
    )
    .unwrap();
    let events = Rc::new(RefCell::new(Vec::new()));
    if input.callbacks {
        let capture = Rc::clone(&events);
        list.set_on_select(Some(Rc::new(move |id| {
            capture
                .borrow_mut()
                .push(vec![Value::from("select"), Value::from(id)]);
        })));
        let capture = Rc::clone(&events);
        list.set_on_cancel(Some(Rc::new(move || {
            capture.borrow_mut().push(vec![Value::from("cancel")]);
        })));
        let capture = Rc::clone(&events);
        list.set_on_label_edit(Some(Rc::new(move |id, label| {
            let mut event = vec![Value::from("edit"), Value::from(id)];
            event.extend(label);
            capture.borrow_mut().push(event);
        })));
    }
    assert_eq!(case.expected.frames.len(), input.ops.len() + 1);
    frame(&list, input, &case.expected.frames[0], &events);
    for (op, expected) in input.ops.iter().zip(&case.expected.frames[1..]) {
        apply_operation(&list, op);
        frame(&list, input, expected, &events);
    }
}
/// Rejects unknown fields, duplicate complete inputs and unreached groups.
pub fn group(name: &str) {
    let _guard = registry::REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = registry::Restore(get_keybindings());
    let raw: Value =
        serde_json::from_str(include_str!("../fixtures/tree-folding-and-search.json")).unwrap();
    let mut inputs = HashSet::new();
    for case in raw.as_array().unwrap() {
        assert!(
            inputs.insert(compact_json(&case["input"]).unwrap()),
            "duplicate complete input"
        );
    }
    let cases: Vec<Case> = serde_json::from_value(raw).unwrap();
    let groups: HashSet<_> = cases.iter().map(|c| &c.test).collect();
    assert_eq!(groups.len(), 27);
    let source = include_str!("../tree_folding_and_search.rs");
    for group in groups {
        assert!(source.contains(&format!("fn {group}()")));
    }
    let selected: Vec<_> = cases.iter().filter(|c| c.test == name).collect();
    assert!(!selected.is_empty());
    for case in selected {
        eprintln!("scenario: {}", case.name);
        replay(case);
    }
}

/// Recorded identity, lifetime, callback and key-definition observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retention {
    pub identity: Identity,
    pub transcript: Vec<Vec<String>>,
    pub surviving: Surviving,
    pub definitions: Vec<(String, Definition)>,
}
/// Every identity distinction recorded from the source owner.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub same_node: Value,
    pub same_entry: Value,
    pub child_ids: Vec<String>,
    pub label_present: Value,
    pub label: Value,
    pub label_after_alias_edit: Value,
    pub entry_alias_shared: Value,
    pub entry_alias_distinct_nodes: Value,
    pub entry_alias_labels_independent: Value,
}
/// Complete retained records and raw metadata after owner teardown.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Surviving {
    pub entry: Value,
    pub children: Vec<Value>,
    pub label: Value,
}
/// Exact action shape and description.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Definition {
    pub default_keys: Binding,
    pub description: String,
}
/// Strictly decodes the recorded retained-owner corpus.
pub fn retention() -> Retention {
    serde_json::from_str(include_str!("../fixtures/tree-retention.json")).unwrap()
}
/// Serializes and restores global key changes through the entire test transaction.
pub fn registry() -> (std::sync::MutexGuard<'static, ()>, registry::Restore) {
    let guard = registry::REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let restore = registry::Restore(get_keybindings());
    set_keybindings(KeybindingsManager::new(KEYBINDINGS.clone(), vec![]));
    (guard, restore)
}
/// Compares default definitions without erasing their scalar/list distinction.
pub fn definitions() {
    let manager = KeybindingsManager::new(KEYBINDINGS.clone(), vec![]);
    for (id, definition) in retention().definitions {
        let actual = manager.get_definition(&id).unwrap();
        assert_eq!(
            actual.description.as_deref(),
            Some(definition.description.as_str())
        );
        match (actual.default_keys, definition.default_keys) {
            (KeybindingKeys::Single(actual), Binding::Single(expected)) => {
                assert_eq!(actual, expected);
            }
            (KeybindingKeys::Multiple(actual), Binding::Multiple(expected)) => {
                assert_eq!(actual, expected);
            }
            _ => panic!("default key shape changed"),
        }
    }
}

/// Builds the recorded root/child relationship with raw root metadata.
pub fn retention_tree<'a>(
    fixture: &'a Surviving,
    metadata: Option<&'a Value>,
) -> Vec<maestro_app::SessionTreeNode<'a>> {
    vec![maestro_app::SessionTreeNode {
        entry: &fixture.entry,
        children: vec![maestro_app::SessionTreeNode {
            entry: &fixture.children[0],
            children: vec![],
            label: None,
            label_timestamp: None,
        }],
        label: metadata,
        label_timestamp: metadata,
    }]
}
