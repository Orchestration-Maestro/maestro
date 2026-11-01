//! Strict public-operation recording replay.
pub mod runtime;
use super::{manual_runtime, published, recording_terminal};
use maestro_chat::BashExecutionComponent;
use maestro_theme::{ColorMode, ColorValue, Theme, ThemeColor, ThemeOptions};
use maestro_tui::{Component, TUI, TerminalImage};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
};

/// Complete recording with theme inputs.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    palettes: BTreeMap<String, Palette>,
    cases: Vec<Case>,
}
/// Exact palette operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Palette {
    bash_mode: u8,
    muted: u8,
    warning: u8,
    error: u8,
    dim: u8,
}
/// Attributed recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    name: String,
    input: Input,
    expected: Expected,
    /// Complete macOS layout when its key names differ.
    expected_macos: Option<Expected>,
}
/// Constructor and ordered public operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    command: Option<String>,
    excluded: Option<bool>,
    terminal_columns: Option<usize>,
    brand_mode: Option<String>,
    observe_runtime: Option<bool>,
    bindings: Option<Bindings>,
    ops: Vec<Operation>,
}
/// Ordered registry operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bindings {
    expand: Vec<String>,
    cancel: Vec<String>,
}
impl Default for Bindings {
    fn default() -> Self {
        Self {
            expand: vec!["ctrl+o".into()],
            cancel: vec!["esc".into()],
        }
    }
}
/// One externally tagged operation rejects extra fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
enum Operation {
    Append(String),
    Complete(Complete),
    Expand(bool),
    Invalidate(bool),
    Palette(String),
    Keys(Bindings),
    Advance(u64),
    CaptureChildren(bool),
    Clear(bool),
    AddAlias(String),
    Add(String),
    RemoveFirst(bool),
    Render(usize),
}
/// Optional final display operands.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Complete {
    exit_code: Option<f64>,
    cancelled: Option<bool>,
    truncated: Option<bool>,
    path: Option<String>,
}
/// Entire rendered transcript and cleaned output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    frames: Vec<Vec<String>>,
    output: String,
    runtime: Option<RuntimeExpected>,
}
/// Observable animation and writer scheduling.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeExpected {
    timers: usize,
    redraws: usize,
}
/// Publishes the registry through its existing admission owner.
fn bindings(keys: &Bindings) {
    let mut definitions = maestro_tui::TUI_KEYBINDINGS.clone();
    definitions.extend(
        [
            ("app.tools.expand", &keys.expand),
            ("tui.select.cancel", &keys.cancel),
        ]
        .map(|(action, keys)| {
            (
                action.into(),
                maestro_tui::KeybindingDefinition {
                    default_keys: maestro_tui::KeybindingKeys::Multiple(keys.clone()),
                    description: None,
                },
            )
        }),
    );
    maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(definitions, vec![]));
}
/// Prepares synthetic or shipped palette without substituting output expectations.
fn theme(palette: &Palette, brand: Option<&str>) -> Rc<Theme> {
    if let Some(brand) = brand {
        return Rc::new(
            maestro_theme::load_theme_from_path(
                &format!(
                    "{}/../maestro-theme/assets/theme/{brand}.json",
                    env!("CARGO_MANIFEST_DIR")
                ),
                Some(ColorMode::Truecolor),
            )
            .unwrap(),
        );
    }
    Rc::new(
        Theme::new(
            [
                (ThemeColor::BashMode, palette.bash_mode),
                (ThemeColor::Muted, palette.muted),
                (ThemeColor::Warning, palette.warning),
                (ThemeColor::Error, palette.error),
                (ThemeColor::Dim, palette.dim),
            ]
            .map(|(key, value)| (key, ColorValue::Index(value))),
            [],
            ColorMode::Truecolor,
            ThemeOptions::default(),
        )
        .unwrap(),
    )
}
/// Replays one attributed group, preserving every frame and repeated operation.
pub fn replay(name: &str) {
    let _guard = registry_guard();
    let raw: serde_json::Value =
        serde_json::from_str(include_str!("../fixtures/shell_rows.json")).unwrap();
    let mut unique = BTreeSet::new();
    for case in raw["cases"].as_array().unwrap() {
        assert!(
            unique.insert(serde_json::to_string(&case["input"]).unwrap()),
            "duplicate complete input"
        );
    }
    assert_eq!(unique.len(), 64);
    let corpus: Corpus = serde_json::from_value(raw).unwrap();
    assert_eq!(
        corpus
            .cases
            .iter()
            .map(|case| &case.test)
            .collect::<BTreeSet<_>>()
            .len(),
        23
    );
    let selected: Vec<_> = corpus
        .cases
        .into_iter()
        .filter(|case| case.test == name)
        .collect();
    assert!(!selected.is_empty(), "missing replay group");
    for case in selected {
        run(case, &corpus.palettes);
    }
}
/// Executes a case using the public component and controlled host.
fn run(mut case: Case, palettes: &BTreeMap<String, Palette>) {
    if cfg!(target_os = "macos") {
        case.expected = case.expected_macos.take().unwrap_or(case.expected);
    }
    let input = case.input;
    let runtime_requested = case.expected.runtime.is_some();
    bindings(&input.bindings.unwrap_or_default());
    let runtime = Rc::new(runtime::Observer::default());
    let terminal =
        recording_terminal::RecordingTerminal::new(input.terminal_columns.unwrap_or(80), 24);
    let ui = TUI::new(
        terminal.handle(),
        runtime.clone(),
        TerminalImage::default(),
        None,
    );
    let (state, live) = published::published(&theme(&palettes["a"], input.brand_mode.as_deref()));
    let command = input.command.unwrap_or_else(|| "echo hello".into());
    let component = BashExecutionComponent::new(
        command.clone(),
        ui.clone(),
        live,
        input.excluded.unwrap_or(false),
    );
    assert_eq!(component.get_command(), command);
    let mut transcript = Transcript::default();
    for op in input.ops {
        apply(
            &component,
            op,
            &runtime,
            (&state, palettes),
            &mut transcript,
        );
    }
    check(
        &component,
        transcript.frames,
        case.expected,
        &case.name,
        &runtime,
    );
    assert_eq!(input.observe_runtime.unwrap_or(false), runtime_requested);
    component.set_complete(None, false, None, None);
    drop(ui);
}
/// Ordered rendered frames and the surviving old child-array alias.
#[derive(Default)]
struct Transcript {
    /// Every frame in invocation order.
    frames: Vec<Vec<String>>,
    /// Optional inherited array captured before replacement.
    alias: Option<maestro_tui::tui::ChildArray>,
}
/// Applies one input operation without bypassing component boundaries.
fn apply(
    component: &BashExecutionComponent,
    op: Operation,
    runtime: &runtime::Observer,
    styling: (&maestro_theme::ThemeState, &BTreeMap<String, Palette>),
    transcript: &mut Transcript,
) {
    match op {
        Operation::Append(text) => component.append_output(&text).unwrap(),
        Operation::Complete(c) => complete(component, c),
        Operation::Expand(value) => component.set_expanded(value),
        Operation::Invalidate(value) => {
            assert!(value);
            component.invalidate();
        }
        Operation::Palette(name) => styling
            .0
            .set_theme_instance(theme(&styling.1[&name], None))
            .unwrap(),
        Operation::Keys(keys) => bindings(&keys),
        Operation::Advance(delta) => {
            for result in runtime
                .clock
                .advance_to(manual_runtime::ms(runtime.clock.millis() + delta))
            {
                result.unwrap();
            }
        }
        Operation::CaptureChildren(value) => {
            assert!(value);
            transcript.alias = Some(component.children());
        }
        Operation::Clear(value) => {
            assert!(value);
            component.clear();
        }
        Operation::AddAlias(text) => transcript
            .alias
            .as_ref()
            .unwrap()
            .borrow_mut()
            .push(Rc::new(maestro_tui::Text::new(text, 0, 0, None))),
        Operation::Add(text) => {
            component.add_child(Rc::new(maestro_tui::Text::new(text, 0, 0, None)));
        }
        Operation::RemoveFirst(value) => {
            assert!(value);
            let first = component.children().borrow()[0].clone();
            component.remove_child(&first);
        }
        Operation::Render(width) => transcript.frames.push(component.render(width)),
    }
}
/// Consumes optional completion operands at the public seam.
fn complete(component: &BashExecutionComponent, completion: Complete) {
    component.set_complete(
        completion.exit_code,
        completion.cancelled.unwrap_or(false),
        completion.truncated.map(|flag| {
            let mut result =
                maestro_app::truncate_tail("", maestro_app::TruncationOptions::default());
            result.truncated = flag;
            result
        }),
        completion.path,
    );
}
/// Compares complete ordered observations, including controlled host scheduling.
fn check(
    component: &BashExecutionComponent,
    mut frames: Vec<Vec<String>>,
    expected: Expected,
    name: &str,
    runtime: &runtime::Observer,
) {
    for rows in &mut frames {
        for row in rows {
            *row = foreground_blank(row);
        }
    }
    assert_eq!(frames, expected.frames, "{name}");
    assert_eq!(component.get_output(), expected.output, "{name}");
    if let Some(expected) = expected.runtime {
        // Writer work is independently coalesced; drain only already-due frame callbacks.
        for result in runtime
            .clock
            .advance_to(manual_runtime::ms(runtime.clock.millis()))
        {
            result.unwrap();
        }
        assert_eq!(runtime.timers.get(), expected.timers, "{name}");
        assert_eq!(runtime.redraws.get(), expected.redraws, "{name}");
    }
}

/// Removes only invisible foreground color escapes from space-only rendered rows.
fn foreground_blank(row: &str) -> String {
    let mut rest = row;
    let mut plain = String::new();
    while !rest.is_empty() {
        if let Some(after) = rest.strip_prefix("\x1b[") {
            let Some(end) = after.find('m') else {
                return row.into();
            };
            let sgr = &after[..end];
            if !(sgr == "39" || sgr.starts_with("38;5;") || sgr.starts_with("38;2;")) {
                return row.into();
            }
            rest = &after[end + 1..];
        } else if let Some(after) = rest.strip_prefix(' ') {
            plain.push(' ');
            rest = after;
        } else {
            return row.into();
        }
    }
    plain
}
/// Prepares the recorded defaults for the unavailable-theme witness.
pub fn default_bindings() {
    bindings(&Bindings::default());
}

use crate::registry::REGISTRY;
/// Holds registry admission and restores the original registry after cleanup.
pub struct RegistryGuard {
    /// Serial admission across this executable's witnesses.
    _lock: std::sync::MutexGuard<'static, ()>,
    /// Registry restored after the component and its timer have been released.
    original: maestro_tui::KeybindingsManager,
}
impl Drop for RegistryGuard {
    fn drop(&mut self) {
        maestro_tui::set_keybindings(self.original.clone());
    }
}
/// Acquires admission before constructing or publishing a registry.
pub fn registry_guard() -> RegistryGuard {
    RegistryGuard {
        _lock: REGISTRY.lock().unwrap(),
        original: maestro_tui::get_keybindings(),
    }
}
