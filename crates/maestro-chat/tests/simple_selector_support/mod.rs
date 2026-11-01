//! Strict recorded inputs and public-output projections.
#![cfg(not(target_arch = "wasm32"))]
pub use crate::selector::{Empty, Frame, Frames};
pub use crate::simple_selector_cases::*;
use maestro_app::ThinkingLevel;
use maestro_tui::Component;
use serde::Deserialize;
use serde_json::Value;
use std::rc::Rc;

/// Current theme and source inventory.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeInput {
    /// Current name.
    pub current: String,
    /// Registry names.
    pub names: Vec<String>,
}
/// Complete terminal rows.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rows {
    /// Styled output.
    pub lines: Vec<String>,
}
/// A native registry over disposable files, using the directory owner.
#[cfg(not(target_arch = "wasm32"))]
pub fn registry(
    names: &[String],
) -> (
    maestro_test_directory::Scratch,
    Rc<maestro_theme::ThemeState>,
) {
    let scratch = maestro_test_directory::Scratch::new().unwrap();
    for name in names
        .iter()
        .filter(|name| !matches!(name.as_str(), "dark" | "light"))
    {
        std::fs::create_dir_all(scratch.path().join("custom")).unwrap();
        std::fs::write(
            scratch.path().join(format!("custom/{name}.json")),
            crate::theme_document::custom_json(name, "#123456").to_string(),
        )
        .unwrap();
    }
    let state = Rc::new(maestro_theme::ThemeState::new(
        maestro_theme::ThemeDirectories {
            themes_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/../maestro-theme/assets/theme")
                .into(),
            custom_themes_dir: scratch.path().join("custom").to_string_lossy().into_owned(),
        },
    ));
    state.set_theme_instance(colors(0)).unwrap();
    (scratch, state)
}
impl Events {
    /// Name confirmation.
    pub fn select(&self) -> Rc<dyn Fn(&str)> {
        let record = self.record::<String>("select");
        Rc::new(move |value| record(value.to_owned()))
    }
    /// Theme preview.
    pub fn preview(&self) -> Rc<dyn Fn(&str)> {
        let events = self.clone();
        Rc::new(move |value| events.push("preview", Some(Value::from(value))))
    }
}
/// Selector kind used by cross-component recordings.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    /// Thinking picker.
    Thinking,
    /// Theme picker.
    Theme,
    /// Image picker.
    Images,
    /// Message picker.
    Messages,
}
/// Layout query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayoutInput {
    /// Selector kind.
    pub kind: Kind,
    /// Viewport cells.
    pub width: usize,
}
/// Ordered list configuration or input operations.
#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Operation {
    /// Programmatic selection.
    Index { index: isize },
    /// Programmatic filter.
    Filter { filter: String },
    /// Input delivery.
    Key { key: String },
}
/// Configuration sequence.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Operations {
    /// Ordered operations.
    pub ops: Vec<Operation>,
}
/// Full snapshots after each operation.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshots {
    /// Every frame including initial state.
    pub frames: Vec<Snapshot>,
}
/// Theme-window input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ThemeWindow {
    /// Registry input order.
    pub names: Vec<String>,
    /// Current input position.
    #[serde(rename = "currentIndex")]
    pub current_index: usize,
}
/// Discovery-failure recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Failure {
    /// Constructor failed.
    pub failed: bool,
    /// Native not-found cause.
    pub missing_file: bool,
    /// No callbacks.
    pub events: Vec<String>,
}
/// Constructor record accepted by the recorded selector.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    /// Entry identity.
    pub id: String,
    /// Authored text.
    pub text: String,
    /// Accepted but unrendered timestamp.
    pub timestamp: Option<String>,
}
/// Message identity query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageInitial {
    /// Chronological records.
    pub messages: Vec<Message>,
    /// Optional initial identity.
    pub initial: Option<String>,
}
/// Authored text query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextInput {
    /// Text to normalize.
    pub text: String,
}
/// Authored text and viewport query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipInput {
    /// Text to clip.
    pub text: String,
    /// Viewport cells.
    pub width: usize,
}
/// Message window query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageWindow {
    /// Supplied messages.
    pub messages: Vec<Message>,
    /// Initial position.
    pub index: usize,
}
/// Convert fixture records to constructor input, preserving every field.
pub fn messages(
    records: Vec<Message>,
) -> Vec<maestro_chat::components::user_message_selector::UserMessageItem> {
    records
        .into_iter()
        .map(
            |record| maestro_chat::components::user_message_selector::UserMessageItem {
                id: record.id,
                text: record.text,
                timestamp: record.timestamp,
            },
        )
        .collect()
}
/// Source-generated chronological input for interaction recordings.
pub fn entries(
    count: usize,
) -> Vec<maestro_chat::components::user_message_selector::UserMessageItem> {
    (0..count)
        .map(
            |index| maestro_chat::components::user_message_selector::UserMessageItem {
                id: format!("id-{index}"),
                text: format!("message {index}"),
                timestamp: Some((count - index).to_string()),
            },
        )
        .collect()
}
/// Construct a message selector with the controlled runtime.
pub fn message_selector(
    records: Vec<maestro_chat::components::user_message_selector::UserMessageItem>,
    initial: Option<String>,
    events: &Events,
    theme: maestro_theme::LiveTheme,
    runtime: &crate::manual_runtime::ManualRuntime,
) -> maestro_chat::UserMessageSelectorComponent {
    maestro_chat::UserMessageSelectorComponent::new(
        records,
        events.select(),
        events.cancel(),
        maestro_chat::components::user_message_selector::UserMessageSelectorOptions {
            initial_selected_id: initial,
            theme,
            runtime: runtime.handle(),
        },
    )
}
/// Count and ordered key input.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Navigation {
    /// Message count.
    pub n: usize,
    /// Input bytes.
    pub ops: Vec<String>,
}
/// Cross-selector kind query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KindInput {
    /// Component kind.
    pub kind: Kind,
}
/// Public component owners, without erasing their container interface.
pub enum Picker {
    /// Thinking picker.
    Thinking(maestro_chat::ThinkingSelectorComponent),
    /// Theme picker.
    Theme(maestro_chat::ThemeSelectorComponent),
    /// Image picker.
    Images(maestro_chat::ShowImagesSelectorComponent),
    /// Message picker.
    Messages(maestro_chat::UserMessageSelectorComponent),
}
impl Picker {
    /// Shared container interface.
    pub fn container(&self) -> &maestro_tui::Container {
        match self {
            Self::Thinking(value) => value,
            Self::Theme(value) => value,
            Self::Images(value) => value,
            Self::Messages(value) => value,
        }
    }
    /// Focus handle as a real toolkit component.
    pub fn list(&self) -> Rc<dyn Component> {
        match self {
            Self::Thinking(value) => value.get_select_list().clone(),
            Self::Theme(value) => value.get_select_list().clone(),
            Self::Images(value) => value.get_select_list().clone(),
            Self::Messages(value) => value.get_message_list().clone(),
        }
    }
}
/// Construct each recorded cross-component starting state.
pub fn picker(
    kind: Kind,
    message_count: usize,
    state: &maestro_theme::ThemeState,
    events: &Events,
    runtime: &crate::manual_runtime::ManualRuntime,
) -> Picker {
    match kind {
        Kind::Thinking => Picker::Thinking(maestro_chat::ThinkingSelectorComponent::new(
            ThinkingLevel::Low,
            levels(),
            events.thinking(),
            events.cancel(),
            state.theme(),
        )),
        Kind::Theme => Picker::Theme(
            maestro_chat::ThemeSelectorComponent::new(
                "dark",
                events.select(),
                events.cancel(),
                events.preview(),
                state,
            )
            .unwrap(),
        ),
        Kind::Images => Picker::Images(maestro_chat::ShowImagesSelectorComponent::new(
            true,
            events.record("select"),
            events.cancel(),
            state.theme(),
        )),
        Kind::Messages => Picker::Messages(message_selector(
            entries(message_count),
            None,
            events,
            state.theme(),
            runtime,
        )),
    }
}
/// Color-switch output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ColorFrames {
    /// Initial rows.
    pub before: Vec<String>,
    /// Rows after publication.
    pub after: Vec<String>,
}
/// Child-array replacement output.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Children {
    /// Stable getter identity.
    pub identity: bool,
    /// Rows after clearing chrome.
    pub cleared: Vec<String>,
    /// Independently retained list.
    pub retained: Vec<String>,
    /// Rows after restoring original children.
    pub restored: Vec<String>,
}
/// Empty-list continuation scenario.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimerMode {
    /// Empty constructor.
    Empty,
    /// Escape before the deadline.
    CancelBeforeTimer,
    /// Replace mutable list callback.
    ReplaceCallback,
    /// Remove chrome.
    ParentRemoved,
    /// Nonempty constructor.
    Nonempty,
}
/// Timer scenario query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerInput {
    /// Scenario.
    pub mode: TimerMode,
}
/// Construction observation and scheduled delay.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerInitial {
    /// Complete panel rows.
    pub lines: Vec<String>,
    /// Ordered notifications.
    pub events: Vec<Vec<Value>>,
    /// Requested delays in milliseconds.
    pub delays: Vec<u64>,
}
/// Continuation observations at each boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TimerOutput {
    /// Construction state.
    pub initial: TimerInitial,
    /// Events at 99 milliseconds.
    pub before: Vec<Vec<Value>>,
    /// Events at 100 milliseconds.
    pub at: Vec<Vec<Value>>,
    /// Events after a further 100 milliseconds.
    pub after: Vec<Vec<Value>>,
    /// Outstanding continuations.
    pub pending: usize,
}

/// Drive the controlled clock and surface each continuation failure.
pub fn advance(runtime: &crate::manual_runtime::ManualRuntime, millis: u64) {
    for result in runtime.advance_to(crate::manual_runtime::ms(millis)) {
        result.unwrap();
    }
}
