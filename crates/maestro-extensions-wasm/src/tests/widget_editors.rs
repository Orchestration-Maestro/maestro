//! Editors and custom editors both test hosts keep, with the nested calls their input and text
//! changes make.
//!
//! An editor renders its border style over `edit|<width>`, then each line of its text. Input
//! names a key: `enter` submits and clears the text unless submission is disabled, any other
//! key is appended. A custom editor offers input to its interceptor first and stops when the
//! interceptor consumed it; then `ctrl+v` pastes an image, `esc` interrupts, `ctrl+d` exits
//! when the text is empty, and an action whose keys its manager binds to the input runs.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use std::collections::HashMap;

use serde_json::json;

use super::observed::Observed;
use super::widget_host::{Paint, Part};
use super::widget_lists::{Call, Event};
use crate::component_adapter::HandlerSlot;

/// The state of one editor.
pub struct Editor {
    /// The text.
    pub text: String,
    /// Texts added to the history.
    pub history: Vec<String>,
    /// Side padding.
    pub padding: f64,
    /// Completion rows.
    pub max_visible: f64,
    /// Whether submission is disabled.
    pub disable_submit: bool,
    /// Whether the editor has the focus.
    pub focused: bool,
    /// Border style.
    pub border: Paint,
    /// A custom editor's keybindings manager.
    pub keys: Option<u32>,
    /// A custom editor's actions with their handlers, in insertion order.
    pub actions: Vec<(String, u32)>,
    /// Handlers by slot name.
    pub handlers: HashMap<&'static str, u32>,
}

impl Editor {
    /// A new editor with `border`, options and an optional manager.
    pub fn new(border: Paint, (padding, max_visible): (Option<f64>, Option<f64>)) -> Self {
        Self {
            text: String::new(),
            history: Vec::new(),
            padding: padding.unwrap_or(0.0),
            max_visible: max_visible.unwrap_or(5.0),
            disable_submit: false,
            focused: false,
            border,
            keys: None,
            actions: Vec::new(),
            handlers: HashMap::new(),
        }
    }
}

/// Editors by widget key.
#[derive(Default)]
pub struct Editors(HashMap<u32, Editor>);

/// Keeps an editor under a widget key.
pub fn keep(observed: &mut Observed, key: u32, editor: Editor) {
    observed.ui.widget_editors.0.insert(key, editor);
}

/// The editor under `key`.
///
/// # Errors
/// Returns a message when the widget is no editor.
pub fn editor(observed: &mut Observed, key: u32) -> Result<&mut Editor, String> {
    observed
        .ui
        .widget_editors
        .0
        .get_mut(&key)
        .ok_or_else(|| format!("widget {key} is no editor"))
}

/// Records a border style read and answers the role of a host style, or names an authored one.
///
/// # Errors
/// Returns a message when the widget is no editor.
pub fn border_color(observed: &mut Observed, key: u32) -> Result<String, String> {
    observed.log(format!("widget {key} borderColor"));
    Ok(match &editor(observed, key)?.border {
        Paint::Host(role) => role.clone(),
        Paint::Authored(id) => format!("authored {id}"),
    })
}

/// Whether the widget under `key` is an editor.
pub fn is_editor(observed: &Observed, key: u32) -> bool {
    observed.ui.widget_editors.0.contains_key(&key)
}

/// The pieces of rendering an editor, or `None` for another widget.
pub fn render(observed: &Observed, key: u32, width: u32) -> Option<Vec<Part>> {
    let editor = observed.ui.widget_editors.0.get(&key)?;
    let border = match &editor.border {
        Paint::Host(role) => Part::Row(format!("<{role}>edit|{width}")),
        Paint::Authored(id) => Part::Style(*id, format!("edit|{width}")),
    };
    let lines = editor
        .text
        .split('\n')
        .map(|line| Part::Row(line.to_owned()));
    Some(std::iter::once(border).chain(lines).collect())
}

/// The nested call of one handler, when the handler is set.
fn handler(editor: &Editor, slot: &str, event: Event) -> Option<Call> {
    editor.handlers.get(slot).map(|&id| Call::Widget(id, event))
}

/// Records one change of the text and returns the change handler's call.
///
/// # Errors
/// Returns a message when the widget is no editor.
pub fn edit(
    observed: &mut Observed,
    key: u32,
    text: &str,
    insert: bool,
) -> Result<Vec<Call>, String> {
    let verb = if insert { "insertText" } else { "setText" };
    observed.log(format!("widget {key} {verb} {}", json!(text)));
    let editor = editor(observed, key)?;
    if insert {
        editor.text.push_str(text);
    } else {
        text.clone_into(&mut editor.text);
    }
    let change = Event::Text(editor.text.clone());
    Ok(handler(editor, "change", change).into_iter().collect())
}

/// Records one input and returns the first calls it makes; an interceptor's call comes alone,
/// and [`proceed`] continues when the interceptor did not consume the input.
///
/// # Errors
/// Returns a message when the widget is no editor.
pub fn input(observed: &mut Observed, key: u32, data: &str) -> Result<Vec<Call>, String> {
    observed.log(format!("widget {key} input {}", json!(data)));
    let editor = editor(observed, key)?;
    if editor.keys.is_some()
        && let Some(call) = handler(
            editor,
            "extensionShortcut",
            Event::Shortcut(data.to_owned()),
        )
    {
        return Ok(vec![call]);
    }
    proceed(observed, key, data)
}

/// The calls an input makes after the interceptor, if any, did not consume it.
///
/// # Errors
/// Returns a message when the widget is no editor.
pub fn proceed(observed: &mut Observed, key: u32, data: &str) -> Result<Vec<Call>, String> {
    let manager = editor(observed, key)?.keys;
    let actions = editor(observed, key)?.actions.clone();
    let bound = |observed: &mut Observed, action: &str| {
        manager.is_some_and(|manager| super::widget_keys::binds(observed, manager, action, data))
    };
    let matched = actions
        .iter()
        .find(|(action, _)| {
            !["app.interrupt", "app.exit"].contains(&action.as_str()) && bound(observed, action)
        })
        .map(|(_, id)| *id);
    let editor = editor(observed, key)?;
    let action = |name: &str| {
        let id = editor.actions.iter().find(|(action, _)| action == name);
        id.map(|(_, id)| Call::Widget(*id, Event::Notice))
    };
    let call = match (manager, data) {
        (Some(_), "ctrl+v") => handler(editor, "pasteImage", Event::Notice),
        (Some(_), "esc") => {
            handler(editor, "escape", Event::Notice).or_else(|| action("app.interrupt"))
        }
        (Some(_), "ctrl+d") if editor.text.is_empty() => {
            handler(editor, "ctrlD", Event::Notice).or_else(|| action("app.exit"))
        }
        _ if matched.is_some() => matched.map(|id| Call::Widget(id, Event::Notice)),
        (_, "enter") if !editor.disable_submit => {
            let text = std::mem::take(&mut editor.text);
            handler(editor, "submit", Event::Text(text))
        }
        (_, "enter") => None,
        _ => {
            editor.text.push_str(data);
            handler(editor, "change", Event::Text(editor.text.clone()))
        }
    };
    Ok(call.into_iter().collect())
}

/// The name the transcript and the handler map give a handler slot.
pub fn slot_name(slot: HandlerSlot) -> &'static str {
    match slot {
        HandlerSlot::Select => "select",
        HandlerSlot::Cancel => "cancel",
        HandlerSlot::SelectionChange => "selectionChange",
        HandlerSlot::Abort => "abort",
        HandlerSlot::Submit => "submit",
        HandlerSlot::Change => "change",
        HandlerSlot::Escape => "escape",
        HandlerSlot::CtrlD => "ctrlD",
        HandlerSlot::PasteImage => "pasteImage",
        HandlerSlot::ExtensionShortcut => "extensionShortcut",
    }
}

/// The recorded operands of a new editor.
pub fn operands(
    border: &Paint,
    list: &[Paint],
    keys: Option<u32>,
    (padding, max_visible): (Option<f64>, Option<f64>),
) -> serde_json::Value {
    let list: Vec<_> = list.iter().map(|paint| format!("{paint:?}")).collect();
    json!({ "border": format!("{border:?}"), "selectList": list, "keys": keys,
        "paddingX": padding, "autocompleteMaxVisible": max_visible })
}

/// One editor request in its owned form.
pub enum Request {
    /// Replaces the text, or inserts at the cursor; the change handler runs.
    Edit(String, bool),
    /// The text.
    GetText,
    /// The expanded text.
    GetExpandedText,
    /// The lines.
    GetLines,
    /// The cursor.
    GetCursor,
    /// Adds to the history.
    AddToHistory(String),
    /// The side padding.
    GetPaddingX,
    /// Replaces the side padding.
    SetPaddingX(f64),
    /// The completion rows.
    GetAutocompleteMaxVisible,
    /// Replaces the completion rows.
    SetAutocompleteMaxVisible(f64),
    /// Whether submission is disabled.
    GetDisableSubmit,
    /// Disables or enables submission.
    SetDisableSubmit(bool),
    /// Whether the completion list shows.
    IsShowingAutocomplete,
    /// Whether the editor has the focus.
    GetFocused,
    /// Gives or takes the focus.
    SetFocused(bool),
    /// Replaces the border style.
    SetBorderColor(Paint),
}

/// What an editor request answers.
pub enum Answer {
    /// Nothing.
    Done,
    /// Text.
    Text(String),
    /// Lines.
    Lines(Vec<String>),
    /// A line and column.
    Cursor(u32, u32),
    /// A number.
    Number(f64),
    /// A flag.
    Flag(bool),
}

/// The transcript line of a request that changes the editor; reads are not recorded.
fn line(request: &Request) -> Option<String> {
    Some(match request {
        Request::AddToHistory(text) => format!("addToHistory {}", json!(text)),
        Request::SetPaddingX(padding) => format!("setPaddingX {padding}"),
        Request::SetAutocompleteMaxVisible(max) => format!("setAutocompleteMaxVisible {max}"),
        Request::SetDisableSubmit(disabled) => format!("setDisableSubmit {disabled}"),
        Request::SetFocused(focused) => format!("setFocused {focused}"),
        Request::SetBorderColor(paint) => format!("setBorderColor {paint:?}"),
        _ => return None,
    })
}

/// Records and answers one request that makes no nested call.
///
/// # Errors
/// Returns a message when the widget is no editor or the request changes the text.
pub fn request(observed: &mut Observed, key: u32, request: Request) -> Result<Answer, String> {
    if let Some(line) = line(&request) {
        observed.log(format!("widget {key} {line}"));
    }
    let editor = editor(observed, key)?;
    let lines = || {
        editor
            .text
            .split('\n')
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    Ok(match request {
        Request::Edit(..) => return Err("a text change makes nested calls".to_owned()),
        Request::GetText | Request::GetExpandedText => Answer::Text(editor.text.clone()),
        Request::GetLines => Answer::Lines(lines()),
        Request::GetCursor => {
            let lines = lines();
            let line = u32::try_from(lines.len() - 1).map_err(|error| error.to_string())?;
            let col = lines.last().map_or(0, String::len);
            Answer::Cursor(line, u32::try_from(col).map_err(|error| error.to_string())?)
        }
        Request::GetPaddingX => Answer::Number(editor.padding),
        Request::GetAutocompleteMaxVisible => Answer::Number(editor.max_visible),
        Request::GetDisableSubmit => Answer::Flag(editor.disable_submit),
        Request::IsShowingAutocomplete => Answer::Flag(false),
        Request::GetFocused => Answer::Flag(editor.focused),
        change => {
            apply(editor, change);
            Answer::Done
        }
    })
}

/// Applies a request that changes the editor without a nested call.
fn apply(editor: &mut Editor, change: Request) {
    match change {
        Request::AddToHistory(text) => editor.history.push(text),
        Request::SetPaddingX(padding) => editor.padding = padding.floor().max(0.0),
        Request::SetAutocompleteMaxVisible(max) => {
            editor.max_visible = max.floor().clamp(3.0, 20.0);
        }
        Request::SetDisableSubmit(disabled) => editor.disable_submit = disabled,
        Request::SetFocused(focused) => editor.focused = focused,
        Request::SetBorderColor(paint) => editor.border = paint,
        _ => {}
    }
}

/// Records and keeps a custom editor's action handler, keeping a replaced action's place.
///
/// # Errors
/// Returns a message when the widget is no editor.
pub fn on_action(
    observed: &mut Observed,
    key: u32,
    action: &str,
    handler: u32,
) -> Result<(), String> {
    observed.log(format!("widget {key} onAction {}", json!(action)));
    let editor = editor(observed, key)?;
    match editor.actions.iter_mut().find(|(bound, _)| bound == action) {
        Some(bound) => bound.1 = handler,
        None => editor.actions.push((action.to_owned(), handler)),
    }
    Ok(())
}
