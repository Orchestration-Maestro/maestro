#![cfg(not(target_arch = "wasm32"))]
//! The recorded footer rows and the controlled world each one describes.
use crate::footer_support::{Roles, World};
use maestro_app::Local;
use maestro_chat::FooterComponent;
use maestro_tui::visible_width;
use serde::Deserialize;
use serde_json::Value;
use std::rc::Rc;

/// One recorded behaviour row.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// The behaviour group the row belongs to.
    pub test: String,
    /// What the row drives.
    pub input: Input,
    /// What the reference produced.
    pub expected: Value,
}

/// What a row drives.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Input {
    /// A footer render over a supplied state.
    Render(Box<Render>),
    /// A token total through the context window spelling.
    Tokens {
        /// The reported total.
        count: f64,
    },
    /// A status text through the status row.
    Sanitize {
        /// The supplied text.
        value: String,
    },
    /// A paint with no published theme.
    UninitializedTheme,
    /// Session replacement with the setters and the no-op lifecycle.
    Lifecycle,
    /// A getter that changes the live world before it answers.
    Callback {
        /// Which getter runs the change.
        phase: String,
    },
    /// A theme republished during a paint.
    LiveTheme,
    /// Metadata changed after the no-op lifecycle calls.
    LiveMetadata,
}

/// A footer render's supplied state.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Render {
    /// The viewport width.
    pub width: usize,
    /// The retained message records.
    #[serde(default)]
    pub entries: Vec<Value>,
    /// The session name.
    #[serde(default)]
    pub name: Option<String>,
    /// The branch: absent is `main`, null is none.
    #[serde(default, deserialize_with = "present")]
    pub branch: Option<Value>,
    /// The model identifier.
    #[serde(default, rename = "modelId")]
    pub model_id: Option<String>,
    /// The model provider.
    #[serde(default)]
    pub provider: Option<String>,
    /// Whether the model supports reasoning.
    #[serde(default)]
    pub reasoning: Option<bool>,
    /// The selected thinking level.
    #[serde(default)]
    pub thinking: Option<String>,
    /// The model's context window.
    #[serde(default, rename = "modelWindow")]
    pub model_window: Option<f64>,
    /// The context usage's window.
    #[serde(default)]
    pub window: Option<f64>,
    /// The context usage's percentage: absent is 12.3, null is unknown.
    #[serde(default, deserialize_with = "present")]
    pub percent: Option<Value>,
    /// Whether the session reports no context usage at all.
    #[serde(default, rename = "contextAbsent")]
    pub context_absent: Option<bool>,
    /// The supplied provider count.
    #[serde(default)]
    pub providers: Option<f64>,
    /// Whether the selected model is on a subscription.
    #[serde(default)]
    pub subscription: Option<bool>,
    /// The auto-compaction setting.
    #[serde(default)]
    pub auto: Option<bool>,
    /// The captured working directory.
    #[serde(default)]
    pub cwd: Option<String>,
    /// The extension statuses in insertion order.
    #[serde(default)]
    pub statuses: Option<Vec<(String, String)>>,
    /// The `HOME` value; absent and null both mean unset.
    #[serde(default, rename = "HOME")]
    pub home: Option<Value>,
    /// The `USERPROFILE` value; absent and null both mean unset.
    #[serde(default, rename = "USERPROFILE")]
    pub userprofile: Option<Value>,
}

/// The lines a render row records, with their measured display widths.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lines {
    /// The returned rows.
    pub lines: Vec<String>,
    /// The display width of each returned row.
    pub widths: Vec<usize>,
}

/// An answer that reports context tokens and nothing the footer totals.
pub fn unreported() -> Value {
    serde_json::json!({
        "role": "assistant",
        "usage": {
            "input": 0.0,
            "output": 0.0,
            "cacheRead": 0.0,
            "cacheWrite": 0.0,
            "totalTokens": 5.0,
            "cost": {"input": 0.0, "output": 0.0, "cacheRead": 0.0, "cacheWrite": 0.0, "total": 0.0}
        }
    })
}

/// Keeps a recorded null apart from a recorded absence.
fn present<'de, D>(deserializer: D) -> Result<Option<Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Value::deserialize(deserializer).map(Some)
}

/// Every recorded row, in recording order.
pub fn rows() -> Vec<Row> {
    serde_json::from_str(include_str!("../fixtures/footer_rendering.json"))
        .expect("the recorded footer corpus")
}

/// The rows of one behaviour group.
pub fn rows_of(test: &str) -> Vec<Row> {
    rows().into_iter().filter(|row| row.test == test).collect()
}

/// The recorded line sets of a bespoke group.
pub fn lines_of(expected: &Value, name: &str) -> Vec<String> {
    serde_json::from_value(expected[name].clone()).expect("a recorded line set")
}

impl Render {
    /// The world this row describes, with the recorded state already applied.
    pub fn world(&self) -> World {
        let cwd = self.cwd.as_deref().unwrap_or("/tmp/project");
        let provider = self.provider.as_deref().unwrap_or("test");
        let subscribed = self.subscription.unwrap_or(false);
        let world = World::new(cwd, subscribed.then_some(provider));
        world.set_model(
            self.model_id.as_deref().unwrap_or("test-model"),
            provider,
            self.reasoning.unwrap_or(false),
            self.model_window.or(self.window).unwrap_or(200_000.0),
        );
        if let Some(level) = self
            .thinking
            .as_deref()
            .and_then(|level| level.parse().ok())
        {
            world.set_thinking(level);
        }
        if let Some(name) = self.name.as_deref().filter(|name| !name.is_empty()) {
            world.set_name(name);
        }
        let branch = match self.branch.as_ref() {
            None => Some("main".to_owned()),
            Some(Value::String(branch)) => Some(branch.clone()),
            Some(_) => None,
        };
        world.metadata.set_branch(Rc::new(move || branch.clone()));
        world.metadata.set_count(Rc::new({
            let count = self.providers.unwrap_or(1.0);
            move || count
        }));
        if let Some(pairs) = &self.statuses {
            world.metadata.set_statuses(pairs);
        }
        world.set_environment(self.home.as_ref(), self.userprofile.as_ref());
        world.append_entries(&self.entries);
        if !self.context_absent.unwrap_or(false) {
            if let Some(tokens) = self.context_tokens() {
                world.set_context(tokens);
            } else {
                world.append_message(&unreported());
                world.append_compaction();
            }
        }
        world
    }

    /// The context tokens this row's percentage implies, when it states a percentage.
    fn context_tokens(&self) -> Option<f64> {
        let window = self.window.unwrap_or(200_000.0);
        match &self.percent {
            None => Some(12.3 / 100.0 * window),
            Some(Value::Number(number)) => Some(number.as_f64()? / 100.0 * window),
            Some(_) => None,
        }
    }

    /// Applies the row's auto-compaction setting to the footer under test.
    pub fn apply_auto(&self, footer: &FooterComponent<Local>) {
        if let Some(auto) = self.auto {
            footer.set_auto_compact_enabled(auto);
        }
    }
}

/// Asserts the rows and their display widths against the recorded expectations.
pub fn assert_lines(actual: &[String], render: &Render, expected: &Lines, roles: &Roles) {
    let frame = format!("width {}", render.width);
    assert_frame(roles, actual, &expected.lines, &frame);
    for (index, line) in actual.iter().enumerate() {
        assert_eq!(
            visible_width(line),
            expected.widths[index],
            "{frame} row {index} width"
        );
    }
}

/// Asserts a frame's rows against the recorded rows, which it must match in number.
pub fn assert_frame(roles: &Roles, actual: &[String], recorded: &[String], frame: &str) {
    assert_eq!(actual.len(), recorded.len(), "{frame} row count");
    for (index, (line, recorded)) in actual.iter().zip(recorded).enumerate() {
        assert_eq!(
            roles.label(line),
            roles.label(recorded),
            "{frame} row {index}"
        );
    }
}

/// A footer's rows as plain text, without the recorded role names.
pub fn plain(line: &str) -> String {
    let mut text = String::new();
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character != '\u{1b}' {
            text.push(character);
            continue;
        }
        if characters.next() != Some('[') {
            continue;
        }
        for next in characters.by_ref() {
            if next.is_ascii_alphabetic() {
                break;
            }
        }
    }
    text
}
