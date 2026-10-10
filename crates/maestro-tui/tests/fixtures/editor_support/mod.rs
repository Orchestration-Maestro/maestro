#![cfg(test)]
//! Public editor observations with controlled terminal effects.
mod host;
pub use host::*;
use maestro_tui::{Component, Editor, EditorOptions, Focusable, tui::InputHandler};
use serde::Deserialize;
use std::{cell::RefCell, rc::Rc};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    test: String,
    id: String,
    config: Config,
    actions: Vec<(String, serde_json::Value)>,
    output: Vec<Observation>,
}
#[derive(Deserialize, serde::Serialize, Default)]
#[serde(deny_unknown_fields)]
struct Config {
    rows: Option<usize>,
    #[serde(default)]
    options: Options,
    bindings: Option<std::collections::BTreeMap<String, serde_json::Value>>,
}
#[derive(Deserialize, serde::Serialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Options {
    padding_x: Option<serde_json::Value>,
    autocomplete_max_visible: Option<serde_json::Value>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {
    text: Option<serde_json::Value>,
    expanded: Option<serde_json::Value>,
    lines: Option<Vec<String>>,
    cursor: Option<Cursor>,
    events: Option<Vec<(String, serde_json::Value)>>,
    requests: Option<usize>,
    padding: Option<f64>,
    maximum: Option<usize>,
    result: Option<Vec<String>>,
    widths: Option<Vec<usize>>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Cursor {
    line: usize,
    col: usize,
}
/// Reads a literal string or a list of literals and `[unit, count]` repetitions.
fn text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .map(|part| match part.as_array().map(Vec::as_slice) {
                Some([unit, count]) => unit
                    .as_str()
                    .unwrap()
                    .repeat(usize::try_from(count.as_u64().unwrap()).unwrap()),
                _ => part.as_str().unwrap().to_owned(),
            })
            .collect(),
        other => panic!("{other}"),
    }
}
fn number(value: &serde_json::Value) -> f64 {
    value
        .as_f64()
        .unwrap_or_else(|| match value.as_str().unwrap() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("{other}"),
        })
}
/// Runs distinct controlled queries owned by one named behavior.
pub fn run(name: &str) {
    let _guard = globals();
    let mut cases: Vec<Case> = serde_json::from_str(include_str!("../editor_cases.json")).unwrap();
    cases.extend(
        serde_json::from_str::<Vec<Case>>(include_str!("../editor_navigation_cases.json")).unwrap(),
    );
    cases.extend(
        serde_json::from_str::<Vec<Case>>(include_str!("../editor_paste_cases.json")).unwrap(),
    );
    let mut queries = std::collections::HashSet::new();
    let mut count = 0;
    for case in cases.into_iter().filter(|case| case.test == name) {
        assert!(queries.insert(serde_json::to_string(&(&case.config, &case.actions)).unwrap()));
        configure(case.config.bindings);
        let mut replay = Replay::new(case.config.rows.unwrap_or(24), &case.config.options);
        if name == "editor_lines_and_cursor_are_independent_snapshots" {
            assert!(replay.editor.input_handler().is_some());
            assert!(replay.editor.focusable().is_some());
            assert!(!replay.editor.disable_submit());
        }
        assert_eq!(case.output.len(), case.actions.len() + 1);
        replay.observe(&case.output[0], None, &case.id);
        for ((action, value), expected) in case.actions.iter().zip(&case.output[1..]) {
            let result = replay.action(action, value);
            replay.requests += replay.runtime.pending();
            replay.runtime.settle().unwrap();
            replay.observe(expected, result.as_ref(), &format!("{} {action}", case.id));
        }
        count += 1;
    }
    assert!(count > 0, "missing cases {name}");
}
/// Restores defaults and applies the current query's literal key overrides.
fn configure(bindings: Option<std::collections::BTreeMap<String, serde_json::Value>>) {
    let config = bindings
        .into_iter()
        .flatten()
        .map(|(key, value)| (key, Some(keys(&value))))
        .collect();
    maestro_tui::set_keybindings(maestro_tui::KeybindingsManager::new(
        maestro_tui::TUI_KEYBINDINGS.clone(),
        config,
    ));
}
/// Decodes the fixture's scalar or list binding shape.
fn keys(value: &serde_json::Value) -> maestro_tui::KeybindingKeys {
    if let Some(key) = value.as_str() {
        return maestro_tui::KeybindingKeys::Single(key.to_owned());
    }
    maestro_tui::KeybindingKeys::Multiple(
        value
            .as_array()
            .unwrap()
            .iter()
            .map(|key| key.as_str().unwrap().to_owned())
            .collect(),
    )
}
/// One retained session and the public observations recorded from it.
struct Replay {
    /// Writer retained for the duration of the query.
    _tui: maestro_tui::TUI,
    /// Retained editor under test.
    editor: Editor,
    /// Controlled live row count.
    terminal: recording_terminal::RecordingTerminal,
    /// Controlled frame scheduling.
    runtime: manual_runtime::ManualRuntime,
    /// Callback sequence.
    events: Rc<RefCell<Vec<(String, String)>>>,
    /// Distinct frame requests, settling between operations.
    requests: usize,
}
impl Replay {
    /// Constructs one empty retained editor and callback trace.
    fn new(rows: usize, options: &Options) -> Self {
        let (tui, terminal, runtime) = host(rows);
        let options = EditorOptions {
            padding_x: options.padding_x.as_ref().map(number),
            autocomplete_max_visible: options.autocomplete_max_visible.as_ref().map(number),
        };
        let editor = Editor::new(&tui, theme(), options);
        assert!(editor.on_change().is_none());
        assert!(editor.on_submit().is_none());
        let events = Rc::new(RefCell::new(Vec::new()));
        let changed = events.clone();
        editor.set_on_change(Some(Rc::new(move |text| {
            changed
                .borrow_mut()
                .push(("change".to_owned(), text.to_owned()));
        })));
        let submitted = events.clone();
        editor.set_on_submit(Some(Rc::new(move |text| {
            submitted
                .borrow_mut()
                .push(("submit".to_owned(), text.to_owned()));
        })));
        Self {
            _tui: tui,
            editor,
            terminal,
            runtime,
            events,
            requests: 0,
        }
    }
    /// Invokes one public operation, returning only observable rows or copied lines.
    fn action(&self, action: &str, value: &serde_json::Value) -> Option<Vec<String>> {
        match action {
            "history" => self.editor.add_to_history(&text(value)),
            "set" => self.editor.set_text(&text(value)),
            "insert" => self.editor.insert_text_at_cursor(&text(value)),
            "key" => self.editor.handle_input(&text(value)),
            "render" => {
                return Some(
                    self.editor
                        .render(usize::try_from(value.as_u64().unwrap()).unwrap()),
                );
            }
            "focus" => self.editor.focus_flag().set(value.as_bool().unwrap()),
            "padding" => self.editor.set_padding_x(number(value)),
            "maximum" => self.editor.set_autocomplete_max_visible(number(value)),
            "disable" => {
                self.editor.set_disable_submit(value.as_bool().unwrap());
                assert_eq!(self.editor.disable_submit(), value.as_bool().unwrap());
            }
            "rows" => self
                .terminal
                .resize(80, usize::try_from(value.as_u64().unwrap()).unwrap()),
            "invalidate" => self.editor.invalidate(),
            "lines-copy" => {
                let mut copy = self.editor.get_lines();
                copy[0] = "mutated".into();
                return Some(copy);
            }
            other => panic!("{other}"),
        }
        None
    }
    /// Compares every retained expected field with its public witness.
    fn observe(&self, expected: &Observation, result: Option<&Vec<String>>, id: &str) {
        if let Some(expected) = &expected.text {
            assert_eq!(self.editor.get_text(), text(expected), "{id} text");
        }
        if let Some(expected) = &expected.expanded {
            assert_eq!(
                self.editor.get_expanded_text(),
                text(expected),
                "{id} expanded"
            );
        }
        if let Some(lines) = &expected.lines {
            assert_eq!(self.editor.get_lines(), *lines, "{id} lines");
        }
        if let Some(cursor) = &expected.cursor {
            assert_eq!(
                self.editor.get_cursor(),
                maestro_tui::autocomplete::CursorPosition {
                    line: cursor.line,
                    col: cursor.col
                },
                "{id} cursor"
            );
        }
        let events = self.events.take();
        if let Some(expected) = &expected.events {
            let expected: Vec<_> = expected
                .iter()
                .map(|(kind, value)| (kind.clone(), text(value)))
                .collect();
            assert_eq!(events, expected, "{id} events");
        }
        self.observe_layout(expected, result, id);
    }
    /// Compares layout and property observations independently of edited content.
    fn observe_layout(&self, expected: &Observation, result: Option<&Vec<String>>, id: &str) {
        if let Some(requests) = expected.requests {
            assert_eq!(self.requests, requests, "{id} requests");
        }
        if let Some(padding) = expected.padding {
            assert_eq!(
                self.editor.get_padding_x().to_bits(),
                padding.to_bits(),
                "{id} padding"
            );
        }
        if let Some(maximum) = expected.maximum {
            assert_eq!(
                self.editor.get_autocomplete_max_visible(),
                maximum,
                "{id} maximum"
            );
        }
        if let Some(widths) = &expected.widths {
            assert_eq!(
                result
                    .unwrap()
                    .iter()
                    .map(|row| maestro_tui::visible_width(row))
                    .collect::<Vec<_>>(),
                *widths,
                "{id} widths"
            );
        }
        if let Some(rows) = &expected.result {
            assert_eq!(result, Some(rows), "{id} rows");
        }
    }
}
