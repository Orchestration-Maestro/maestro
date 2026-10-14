//! Strict whole-frame corpus replay through the public list.
#[allow(
    dead_code,
    reason = "Only controlled environment and effect observations are used."
)]
#[path = "../../../maestro-test-terminal/tests/support/manual_runtime.rs"]
mod manual_runtime;
#[path = "../message_support/palette.rs"]
mod palette;
#[allow(dead_code, reason = "Only publication helpers are shared.")]
#[path = "../../../maestro-theme/tests/published/mod.rs"]
mod published;
#[path = "../support/registry.rs"]
mod registry;
#[path = "../support/tree.rs"]
mod tree;
use maestro_app::{
    compact_json, format_read_line_range, presentation_data::keybindings::KEYBINDINGS,
    truncate_utf16,
};
/// The frontend facade retains the shared JSON signature.
const _: fn(&serde_json::Value) -> Result<String, serde_json::Error> = compact_json;
/// The frontend facade retains the shared prefix signature.
const _: fn(&str, usize) -> String = truncate_utf16;
/// The frontend facade retains the shared read-range signature.
const _: fn(&serde_json::Value) -> String = format_read_line_range;
use maestro_chat::components::tree_selector::{FilterMode, TreeList};
use maestro_tui::{KeybindingsManager, get_keybindings, set_keybindings, visible_width};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use tree::{Node, tree};
/// An attributed recorded invocation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    #[serde(rename = "case")]
    name: String,
    input: Input,
    expected: Expected,
}
/// Constructor and render operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    nodes: Vec<Node>,
    roots: Vec<usize>,
    leaf: Option<String>,
    mode: String,
    lines: usize,
    widths: Vec<usize>,
    ops: Vec<Operation>,
    initial: Option<String>,
    env: BTreeMap<String, String>,
    palettes: Vec<String>,
    styles_enabled: bool,
}
/// A key or operation-local host change.
#[derive(Deserialize)]
#[serde(untagged)]
enum Operation {
    Key(String),
    Change(Change),
}
/// Render-only changes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Change {
    env: Option<BTreeMap<String, String>>,
    invalidate: Option<bool>,
}
/// Ordered results ending at the first reached error.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    frames: Vec<Frame>,
    error: Option<Failure>,
}
/// Native error category.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    kind: String,
}
/// One observation after construction or input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    selected: Option<String>,
    rendered: Vec<Rendered>,
}
/// Exact styled lines and independent cell widths.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rendered {
    palette: String,
    width: usize,
    lines: Vec<String>,
    cell_widths: Vec<usize>,
}
/// Convert the recorded mode to its public variant.
fn mode(value: &str) -> FilterMode {
    match value {
        "default" => FilterMode::Default,
        "no-tools" => FilterMode::NoTools,
        "user-only" => FilterMode::UserOnly,
        "labeled-only" => FilterMode::LabeledOnly,
        "all" => FilterMode::All,
        _ => panic!("unknown mode"),
    }
}
/// Observe one frame with the surviving list and theme slot.
fn frame(
    list: &TreeList<'_, '_>,
    input: &Input,
    frame: &Frame,
    host: &manual_runtime::ManualRuntime,
) {
    assert_eq!(
        list.get_selected_node()
            .map(|n| n.entry["id"].as_str().unwrap()),
        frame.selected.as_deref()
    );
    let (state, live) = published::published(&palette::palette(&input.palettes[0]));
    assert_eq!(
        frame.rendered.len(),
        input.palettes.len() * input.widths.len()
    );
    for (expected, (palette, width)) in frame.rendered.iter().zip(
        input
            .palettes
            .iter()
            .flat_map(|p| input.widths.iter().map(move |w| (p, w))),
    ) {
        assert_eq!((&expected.palette, expected.width), (palette, *width));
        state
            .set_theme_instance(self::palette::palette(palette))
            .unwrap();
        let actual = list
            .render(*width, &live, host, input.styles_enabled)
            .unwrap();
        assert_eq!(actual, expected.lines);
        assert_eq!(
            actual
                .iter()
                .map(|line| visible_width(line))
                .collect::<Vec<_>>(),
            expected.cell_widths
        );
        assert!(expected.cell_widths.iter().all(|cells| cells <= width));
    }
}
/// Apply a key or render-local change without altering process environment.
fn operation(
    list: &mut TreeList<'_, '_>,
    operation: &Operation,
    host: &mut manual_runtime::ManualRuntime,
) {
    match operation {
        Operation::Key(key) => list.handle_input(key).unwrap(),
        Operation::Change(change) => {
            assert_ne!(change.env.is_some(), change.invalidate.is_some());
            if let Some(env) = &change.env {
                *host = manual_runtime::ManualRuntime::new();
                for (key, value) in env {
                    host.set_environment(key, value);
                }
            }
            if let Some(invalidate) = change.invalidate {
                assert!(invalidate);
                list.invalidate();
            }
        }
    }
}
/// Replay keys, live environment changes and the first display failure.
fn replay(case: &Case) {
    let input = &case.input;
    let tree = tree(&input.nodes, &input.roots);
    let mut list = TreeList::new(
        &tree,
        input.leaf.as_deref(),
        input.lines,
        input.initial.as_deref(),
        Some(mode(&input.mode)),
    )
    .unwrap();
    let mut host = manual_runtime::ManualRuntime::new();
    for (key, value) in &input.env {
        host.set_environment(key, value);
    }
    for index in 0..=input.ops.len() {
        if index > 0 {
            operation(&mut list, &input.ops[index - 1], &mut host);
        }
        if let Some(expected) = case.expected.frames.get(index) {
            frame(&list, input, expected, &host);
        } else {
            assert_eq!(index, case.expected.frames.len());
            assert_eq!(index, input.ops.len());
            assert_eq!(case.expected.error.as_ref().unwrap().kind, "InvalidData");
            let (_, live) = published::published(&palette::palette(&input.palettes[0]));
            assert_eq!(
                list.render(input.widths[0], &live, &host, input.styles_enabled)
                    .unwrap_err()
                    .kind(),
                std::io::ErrorKind::InvalidData
            );
        }
        assert_eq!(host.pending(), 0);
        assert!(host.files().is_empty());
        assert!(host.take_futures().is_empty());
    }
    assert_eq!(
        case.expected.frames.len() + usize::from(case.expected.error.is_some()),
        input.ops.len() + 1
    );
}
/// Replay exactly one group; reject duplicate and unowned cases.
pub fn group(name: &str) {
    let _guard = registry::REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _restore = registry::Restore(get_keybindings());
    set_keybindings(KeybindingsManager::new(KEYBINDINGS.clone(), vec![]));
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/tree-rendering.json")).unwrap();
    let keys: HashSet<_> = cases.iter().map(|c| (&c.test, &c.name)).collect();
    assert_eq!(keys.len(), cases.len());
    let tests = include_str!("../tree_rendering.rs");
    let groups: HashSet<_> = cases.iter().map(|case| &case.test).collect();
    assert_eq!(groups.len(), 29);
    for group in groups {
        assert!(tests.contains(&format!("fn {group}()")));
    }
    let selected: Vec<_> = cases.iter().filter(|c| c.test == name).collect();
    assert!(!selected.is_empty());
    for case in selected {
        eprintln!("scenario: {}", case.name);
        replay(case);
    }
}
