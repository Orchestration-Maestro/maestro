//! Identical terminal scripts on native exports and the built component.
use super::{Question, ask};
use crate::component_adapter::Exports;
use crate::tests::{
    controlled::Controlled,
    custom_host::Script,
    example_support,
    host::Harness,
    observed::Observed,
    theme_host::{Palette, Reply},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// Adapter selected only by the test runner.
enum Host {
    /// In-process execution of the shipped source.
    Native(Controlled, Exports<Controlled>),
    /// The shipped component with real generated bindings.
    Component(Box<Harness>),
}
impl Host {
    /// Loads the same factory through the selected adapter.
    fn new(component: bool) -> Result<Self, String> {
        if component {
            return Ok(Self::Component(Box::new(Harness::example("question")?)));
        }
        let (host, exports) = crate::tests::block_on(example_support::start::<Question>())?;
        crate::component_adapter::install(std::rc::Rc::new(crate::component_adapter::Builder(
            host.clone(),
        )));
        Ok(Self::Native(host, exports))
    }
    /// Changes observations between tool invocations, never during callbacks.
    fn observe<T>(&mut self, f: impl FnOnce(&mut Observed) -> T) -> T {
        match self {
            Self::Native(host, _) => f(&mut host.observe()),
            Self::Component(host) => f(host.observed()),
        }
    }
    /// Invokes the registered tool, not a private executor.
    fn ask(&mut self, params: Value) -> Result<Value, String> {
        let key = self.observe(|o| o.callback("tool question"))?;
        match self {
            Self::Native(host, exports) => crate::tests::block_on(ask(host, exports, key, params)),
            Self::Component(host) => {
                let ctx = host.ordinary("/work").map_err(|e| e.to_string())?;
                let encoded = example_support::invoke_tool_on_component(
                    host,
                    key,
                    &params,
                    crate::tests::host_family::guest::ToolCapabilities {
                        ctx,
                        signal: None,
                        update: None,
                    },
                )?;
                serde_json::from_str(&encoded).map_err(|e| e.to_string())
            }
        }
    }
}
/// Full frames captured as literal expected output, including the host editor's rows.
fn frames() -> std::collections::HashMap<String, Vec<String>> {
    serde_json::from_str(include_str!("frames.json")).unwrap()
}
/// Supplies exact theme calls without reproducing a styling algorithm.
fn palette() -> Palette {
    let mut replies = Vec::new();
    for (role, texts) in [
        (
            "accent",
            vec![
                "────────────────────────────────────────────────────────────────────────────────",
                "> ",
                "1. Rust",
                "2. Go",
                "3. Type something.",
                "3. Type something. ✎",
                "edit|78",
            ],
        ),
        (
            "text",
            vec![" Which?", "1. Rust", "2. Go", "3. Type something."],
        ),
        ("muted", vec!["Fast", " Your answer:"]),
        (
            "dim",
            vec![
                " ↑↓ navigate • Enter to select • Esc to cancel",
                " Enter to submit • Esc to go back",
            ],
        ),
    ] {
        replies.extend(texts.into_iter().map(|text| Reply {
            method: "fg".into(),
            operands: json!([role, text]),
            answer: json!({"ok":text}),
        }));
    }
    Palette {
        name: Some("dark".into()),
        source_path: None,
        source_info: None,
        mode: "truecolor".into(),
        replies,
    }
}
/// Installs literal helper replies and one bounded terminal input script.
fn script(host: &mut Host, inputs: &[&str], states: &[&str]) {
    let frames = frames();
    let mut replies = serde_json::Map::new();
    for row in frames.values().flatten() {
        replies.insert(
            format!("truncate_to_width {}", json!([row, 80, "...", false])),
            json!(row),
        );
    }
    for input in inputs {
        for key in ["escape", "up", "down", "enter"] {
            replies.insert(
                format!("keys.matches_key {}", json!([input, key])),
                json!(input == &key),
            );
        }
    }
    for ch in " snow\u{feff}".chars() {
        replies.insert(
            format!("is_whitespace_char {}", json!([ch.to_string()])),
            json!(ch == ' ' || ch == '\u{feff}'),
        );
    }
    host.observe(|o| {
        o.catalog.data.has_ui = true;
        o.themes.publish("dark", palette());
        o.actions.value = Value::Object(replies);
        o.ui.custom_script = Some(Script {
            width: None,
            columns: 80,
            inputs: inputs.iter().map(|s| (*s).into()).collect(),
            input_errors: BTreeMap::new(),
            frames: states.iter().map(|s| frames[*s].clone()).collect(),
            ticks: vec![],
        });
    });
}
/// The same question with one described choice and one plain choice.
fn params() -> Value {
    json!({"question":"Which?","options":[{"label":"Rust","description":"Fast"},{"label":"Go"}]})
}
/// Asserts complete result records, including omitted versus present custom flags.
fn answer(actual: &Value, text: &str, answer: &Value, custom: Option<bool>) {
    let mut details = json!({"question":"Which?","options":["Rust","Go"],"answer":answer});
    if let Some(custom) = custom {
        details["wasCustom"] = json!(custom);
    }
    assert_eq!(
        actual,
        &json!({"content":[{"type":"text","text":text}],"details":details})
    );
}
/// Navigation clamps, editor Escape clears, blank submit returns, and choices keep numbering.
fn exercise(component: bool) -> Result<(), String> {
    let mut host = Host::new(component)?;
    script(
        &mut host,
        &[
            "up", "down", "down", "down", "enter", "  snow  ", "escape", "enter", " ", "enter",
            "up", "enter",
        ],
        &[
            "first", "first", "second", "custom", "custom", "editing", "typed", "custom",
            "editing", "blank", "custom", "second", "second",
        ],
    );
    answer(
        &host.ask(params())?,
        "User selected: 2. Go",
        &json!("Go"),
        Some(false),
    );
    script(
        &mut host,
        &["down", "down", "enter", "  snow  ", "enter"],
        &["first", "second", "custom", "editing", "typed", "editing"],
    );
    answer(
        &host.ask(params())?,
        "User wrote: snow",
        &json!("snow"),
        Some(true),
    );
    script(&mut host, &["escape"], &["first", "first"]);
    answer(
        &host.ask(params())?,
        "User cancelled the selection",
        &Value::Null,
        None,
    );
    let empty = host.ask(json!({"question":"Which?","options":[]}))?;
    assert_eq!(
        empty,
        json!({"content":[{"type":"text","text":"Error: No options provided"}],"details":{"question":"Which?","options":[],"answer":null}})
    );
    Ok(())
}
#[test]
fn question_ui_selects_customizes_and_cancels() -> Result<(), String> {
    exercise(false)
}
#[test]
fn question_component_selects_customizes_and_cancels() -> Result<(), String> {
    exercise(true)
}

impl Drop for Host {
    fn drop(&mut self) {
        match self {
            Self::Native(host, _) => {
                example_support::released(host);
            }
            Self::Component(host) => {
                example_support::released_component(host).unwrap();
            }
        }
    }
}
