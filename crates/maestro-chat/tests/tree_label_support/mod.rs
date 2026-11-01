//! Whole-frame recorded panel observations.
mod errors;
use crate::bindings::{self, Binding};
use crate::manual_runtime;
use crate::palette;
use crate::published;
use crate::registry;
mod replay;
use crate::subprocess;
use crate::tree;
mod writer;
use maestro_chat::{TreeSelectorCallbacks, TreeSelectorComponent, TreeSelectorOptions};
use maestro_tui::Component;
use serde::Deserialize;
use serde_json::Value;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

/// One attributed recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    #[serde(rename = "case")]
    name: String,
    input: Input,
    expected: Expected,
}
/// Construction and subsequent operation operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    nodes: Vec<tree::Node>,
    roots: Vec<usize>,
    leaf: Option<String>,
    height: usize,
    mode: String,
    initial: Option<String>,
    ops: Vec<Operation>,
    bindings: std::collections::BTreeMap<String, Binding>,
    widths: Vec<usize>,
    styles: bool,
    palette: String,
    tz: String,
    now: String,
    callback: String,
    clock_reenter: bool,
}
/// Key input or one public operation.
#[derive(Deserialize)]
#[serde(untagged)]
enum Operation {
    Key(String),
    Change(Change),
}
/// Recorded live changes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Change {
    set_bindings: Option<std::collections::BTreeMap<String, Binding>>,
    focus: Option<bool>,
    palette: Option<String>,
    now: Option<String>,
    update: Option<Update>,
    list_key: Option<String>,
    advance: Option<u64>,
    clear_outer: Option<bool>,
    invalidate: Option<bool>,
}
/// One label update; omitted strings remain absent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Update {
    id: String,
    label: Option<String>,
    timestamp: Option<String>,
}
/// Complete ordered observations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    frames: Vec<Frame>,
    error: Option<Failure>,
}
/// Expected caller panic boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    step: usize,
    kind: String,
}
/// Public state before rendering plus ordered output and effects afterward.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Frame {
    selected: Option<String>,
    query: String,
    metadata: Value,
    children: Vec<Option<usize>>,
    events: Vec<Value>,
    rendered: Vec<Rendered>,
    clock_reads: usize,
    pending: usize,
}
/// Exact output for one width.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rendered {
    width: usize,
    panel: Vec<String>,
    list: Vec<String>,
}
/// Mutable controlled effects shared with callbacks, without strong backedges.
struct Effects {
    panel: RefCell<std::rc::Weak<TreeSelectorComponent>>,
    events: RefCell<Vec<Value>>,
    reads: Cell<usize>,
    instant: Cell<std::time::SystemTime>,
    reenter: Cell<bool>,
}

/// Replays all cases belonging to one named public witness in isolated zones.
pub fn group(name: &str) {
    if !maestro_test_process::selection().is_child() {
        for tz in ["UTC", "America/New_York", "Asia/Kathmandu"] {
            subprocess::run_child(
                maestro_test_process::TestProcess::current(name)
                    .unwrap()
                    .case("*"),
                &[("TZ", Some(tz))],
            );
        }
        return;
    }
    let _guard = registry::REGISTRY.lock().unwrap();
    let _restore = registry::Restore(maestro_tui::get_keybindings());
    let cases: Vec<Case> =
        on_this_platform(include_str!("../fixtures/tree-folding-and-labels.json"));
    let tz = std::env::var("TZ").unwrap();
    let selected: Vec<_> = cases
        .iter()
        .filter(|c| c.test == name && c.input.tz == tz)
        .collect();
    for case in selected {
        replay::case(case);
    }
}

/// One recorded fold-hint row and the row macOS renders in its place.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlatformRow {
    recorded: String,
    macos: String,
}

/// The recorded corpus as this platform renders it: macOS names the alt key `option`, so each
/// recorded hint row, including those inside recorded writes, becomes the macOS row laid out
/// for its width.
fn on_this_platform<T: serde::de::DeserializeOwned>(recorded: &str) -> T {
    let mut corpus: Value = serde_json::from_str(recorded).unwrap();
    if cfg!(target_os = "macos") {
        let rows: Vec<PlatformRow> =
            serde_json::from_str(include_str!("../fixtures/tree-macos-rows.json")).unwrap();
        replace_rows(&mut corpus, &rows);
    }
    serde_json::from_value(corpus).unwrap()
}

/// Replaces every recorded row within the strings of `value`.
fn replace_rows(value: &mut Value, rows: &[PlatformRow]) {
    match value {
        Value::String(text) => {
            for row in rows {
                *text = text.replace(&row.recorded, &row.macos);
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| replace_rows(item, rows)),
        Value::Object(fields) => fields
            .values_mut()
            .for_each(|item| replace_rows(item, rows)),
        _ => {}
    }
}

/// Replays original native panel errors.
pub fn error(name: &str) {
    errors::replay(name);
}

/// Replays actual writer effects.
pub fn writer() {
    writer::replay();
}
