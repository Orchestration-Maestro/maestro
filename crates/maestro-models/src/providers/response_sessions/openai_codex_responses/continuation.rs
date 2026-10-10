//! Full-versus-delta request selection and the context a cached connection retains.

use super::CodexError;
use super::sessions::lock;
use super::websocket::{wire_body, wire_members};
use crate::providers::json_text::{compact_json, compact_members};
use crate::providers::responses::openai_responses_shared::messages::convert_assistant;
use crate::{DiagnosticErrorInfo, Model, SharedAssistantMessage, StopReason};
use serde_json::Value;
use std::borrow::Cow;
use std::sync::{Mutex, PoisonError};

/// The request text and the facts the counters read from the request as sent.
pub(super) struct Request {
    /// Socket message text.
    pub(super) wire: String,
    /// Array items of the sent input, or UTF-16 units of a text input.
    pub(super) input_items: usize,
    /// Nonempty previous response identifier of the sent body.
    pub(super) previous_response_id: Option<String>,
    /// The sent body stores its response.
    pub(super) store_true: bool,
}

impl Request {
    /// The request that sends `body` unchanged.
    pub(super) fn full(body: &Value) -> Result<Self, CodexError> {
        Ok(Self {
            wire: wire_body(body)?,
            input_items: match body.get("input") {
                Some(Value::Array(items)) => items.len(),
                Some(Value::String(text)) => text.encode_utf16().count(),
                _ => 0,
            },
            previous_response_id: body
                .get("previous_response_id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty())
                .map(str::to_owned),
            store_true: body.get("store") == Some(&Value::Bool(true)),
        })
    }
}

/// What a cached connection remembers of its last completed request.
pub(super) struct Continuation {
    /// Complete request body that was sent.
    body: Value,
    /// Compact text of that body without `input` and `previous_response_id`.
    rest: String,
    /// Identifier of the response it produced.
    response_id: String,
    /// Input items that response contributes to the next request.
    items: Vec<Value>,
}

/// Compact text of the body members other than `input` and `previous_response_id`.
fn without_input(body: &Value) -> Result<String, serde_json::Error> {
    compact_members(
        body.as_object()
            .into_iter()
            .flatten()
            .filter(|(name, _)| !matches!(name.as_str(), "input" | "previous_response_id"))
            .map(|(name, value)| (name.as_str(), value)),
    )
}

/// The elements an `input` contributes to a baseline: none when absent or null, characters of a
/// text, and no match for a shape that cannot be spread.
fn spread(input: Option<&Value>) -> Option<Vec<Cow<'_, Value>>> {
    match input {
        None | Some(Value::Null) => Some(Vec::new()),
        Some(Value::Array(items)) => Some(items.iter().map(Cow::Borrowed).collect()),
        Some(Value::String(text)) => Some(
            text.chars()
                .map(|character| Cow::Owned(Value::from(character.to_string())))
                .collect(),
        ),
        Some(_) => None,
    }
}

/// The body with the identifier and the input suffix in place of its own members; absent members
/// follow the existing ones, identifier first.
fn delta_wire(body: &Value, id: &Value, input: &Value) -> Result<String, CodexError> {
    let own = body.as_object();
    let replaced = own.into_iter().flatten().map(|(name, value)| {
        let value = match name.as_str() {
            "previous_response_id" => id,
            "input" => input,
            _ => value,
        };
        (name.as_str(), value)
    });
    let appended = [("previous_response_id", id), ("input", input)]
        .into_iter()
        .filter(|(name, _)| own.is_none_or(|own| !own.contains_key(*name)));
    wire_members(replaced.chain(appended))
}

impl Continuation {
    /// Retain `body` with the response it produced; `None` when its text cannot be written.
    pub(super) fn new(body: &Value, response_id: &str, items: Vec<Value>) -> Option<Self> {
        Some(Self {
            rest: without_input(body).ok()?,
            body: body.clone(),
            response_id: response_id.to_owned(),
            items,
        })
    }

    /// The request that sends only what follows the retained prefix, when `body` matches.
    fn delta(&self, body: &Value) -> Option<Request> {
        if self.response_id.is_empty() || without_input(body).ok()? != self.rest {
            return None;
        }
        let current: &[Value] = match body.get("input") {
            None | Some(Value::Null) => &[],
            Some(Value::Array(items)) => items,
            Some(_) => return None,
        };
        let baseline: Vec<Cow<'_, Value>> = spread(self.body.get("input"))?
            .into_iter()
            .chain(self.items.iter().map(Cow::Borrowed))
            .collect();
        let suffix = current.get(baseline.len()..)?;
        for (expected, actual) in baseline.iter().zip(current) {
            if compact_json(expected).ok()? != compact_json(actual).ok()? {
                return None;
            }
        }
        let id = Value::from(self.response_id.as_str());
        let input = Value::Array(suffix.to_vec());
        Some(Request {
            wire: delta_wire(body, &id, &input).ok()?,
            input_items: suffix.len(),
            previous_response_id: Some(self.response_id.clone()),
            store_true: body.get("store") == Some(&Value::Bool(true)),
        })
    }
}

/// The delta request for `body` when the retained context matches; otherwise the context is
/// cleared and the caller sends the full request.
pub(super) fn select(slot: &Mutex<Option<Continuation>>, body: &Value) -> Option<Request> {
    let mut slot = lock(slot);
    let request = slot.as_ref()?.delta(body);
    if request.is_none() {
        *slot = None;
    }
    request
}

/// Input items a finished response contributes to the next request.
pub(super) fn project(
    output: &SharedAssistantMessage,
    model: &Model,
) -> Result<Vec<Value>, DiagnosticErrorInfo> {
    let message = output.read().unwrap_or_else(PoisonError::into_inner);
    if matches!(message.stop_reason, StopReason::Error | StopReason::Aborted) {
        return Ok(Vec::new());
    }
    // This freshly reduced message belongs to this model, so projection needs no cross-model rewrite.
    let mut items = convert_assistant(&message, model, 0)?;
    items.retain(|item| item["type"] != "function_call_output");
    Ok(items)
}

/// Retain the sent `body` and the finished response unless the response has no identifier.
pub(super) fn retain(
    slot: &Mutex<Option<Continuation>>,
    body: &Value,
    output: &SharedAssistantMessage,
    model: &Model,
) -> Result<(), DiagnosticErrorInfo> {
    let id = output
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .response_id
        .clone()
        .filter(|id| !id.is_empty());
    if let Some(id) = id {
        *lock(slot) = Continuation::new(body, &id, project(output, model)?);
    }
    Ok(())
}
