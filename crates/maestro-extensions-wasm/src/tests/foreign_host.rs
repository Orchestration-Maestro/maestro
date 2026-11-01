//! The editor factory another extension set, as both test hosts lend it, and the requests the
//! editors it builds answer.
//!
//! A `custom` factory builds a custom editor that answers every request. A `plain` factory
//! builds an editor with only the required operations: every optional request answers absent.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::json;

use super::observed::Observed;
use super::widget_editors::{self, Request};
use super::widget_host::Paint;
use super::widget_lists::Call;

/// The factory another extension set and the plain editors it built.
#[derive(Default)]
pub struct Foreign {
    /// Kind of the factory, `custom` or `plain`; absent while no other extension set one.
    pub kind: Option<String>,
    /// Keys of the plain editors built.
    plain: Vec<u32>,
}

/// Whether the current foreign factory builds a custom editor, absent without one.
pub fn current(observed: &Observed) -> Option<bool> {
    observed
        .ui
        .foreign
        .kind
        .as_deref()
        .map(|kind| kind == "custom")
}

/// Records the editor a foreign factory built under `key`.
pub fn built(observed: &mut Observed, key: u32, custom: bool) {
    observed.log(format!("foreign built {key} custom={custom}"));
    if !custom {
        observed.ui.foreign.plain.push(key);
    }
}

/// One request on an editor a foreign factory built, in its owned form.
pub enum Op {
    /// The text.
    GetText,
    /// Replaces the text, or inserts at the cursor.
    Edit(String, bool),
    /// Replaces or removes the submission or change handler with an authored one.
    SetHandler(&'static str, Option<u32>),
    /// Whether a change handler is installed.
    OnChange,
    /// Replaces the completion provider with an authored one.
    SetAutocompleteProvider(u32),
    /// The border style.
    BorderColor,
    /// Replaces the border style.
    SetBorderColor(Paint),
    /// A request that makes no nested call.
    Plain(Request),
}

impl Op {
    /// Whether the request is an optional editor operation.
    fn optional(&self) -> bool {
        match self {
            Self::GetText | Self::SetHandler(..) | Self::OnChange => false,
            Self::Edit(_, insert) => *insert,
            _ => true,
        }
    }
}

/// What a request answers.
pub enum Answer {
    /// The operation ran.
    Done,
    /// The editor lacks the operation.
    Absent,
    /// Text.
    Text(String),
    /// Whether a change handler is installed; the host lends one named `foreign`.
    Handler(bool),
    /// The role of the border style.
    Style(String),
    /// The operation ran after these nested calls.
    Calls(Vec<Call>),
}

/// The name the host lends an editor's change handler under.
pub const LENT: &str = "foreign";

/// Records and answers one request on the editor under `key`.
///
/// # Errors
/// Returns a message when the widget is no editor.
pub fn request(observed: &mut Observed, key: u32, op: Op) -> Result<Answer, String> {
    if op.optional() && observed.ui.foreign.plain.contains(&key) {
        observed.log(format!("widget {key} foreign absent"));
        return Ok(Answer::Absent);
    }
    Ok(match op {
        Op::GetText => match widget_editors::request(observed, key, Request::GetText)? {
            widget_editors::Answer::Text(text) => Answer::Text(text),
            _ => return Err("the editor answered no text".to_owned()),
        },
        Op::Edit(text, insert) => {
            Answer::Calls(widget_editors::edit(observed, key, &text, insert)?)
        }
        Op::SetHandler(slot, handler) => {
            observed.log(format!(
                "widget {key} setHandler {slot} {}",
                handler.is_some()
            ));
            let handlers = &mut widget_editors::editor(observed, key)?.handlers;
            match handler {
                Some(handler) => handlers.insert(slot, handler),
                None => handlers.remove(slot),
            };
            Answer::Done
        }
        Op::OnChange => {
            let slot = widget_editors::slot_name(crate::component_adapter::HandlerSlot::Change);
            Answer::Handler(
                widget_editors::editor(observed, key)?
                    .handlers
                    .contains_key(slot),
            )
        }
        Op::SetAutocompleteProvider(provider) => {
            observed.log(format!(
                "widget {key} setAutocompleteProvider {}",
                json!([provider])
            ));
            Answer::Done
        }
        Op::BorderColor => Answer::Style(widget_editors::border_color(observed, key)?),
        Op::SetBorderColor(paint) => {
            widget_editors::request(observed, key, Request::SetBorderColor(paint))?;
            Answer::Done
        }
        Op::Plain(request) => match widget_editors::request(observed, key, request)? {
            widget_editors::Answer::Text(text) => Answer::Text(text),
            _ => Answer::Done,
        },
    })
}
