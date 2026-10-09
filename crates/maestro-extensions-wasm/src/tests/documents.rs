//! The event documents the tests deliver, the value classes they combine, and what a delivery
//! hands back, read as documents.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde_json::{Value, json};

use super::scenario::{Decision, Delivery, Driver, Encoded};

/// Text that must pass through unchanged: a byte order mark, a next-line control, a
/// supplementary character, a parent segment and surrounding spaces.
pub const LITERAL: &str = " \u{feff}\u{85}é😀/../x ";

/// The `type` tag of every delivered event kind.
pub const KINDS: [&str; 23] = [
    "message_update",
    "tool_execution_start",
    "tool_execution_update",
    "tool_execution_end",
    "model_select",
    "thinking_level_select",
    "context",
    "before_agent_start",
    "agent_start",
    "agent_end",
    "turn_start",
    "turn_end",
    "message_start",
    "message_end",
    "resources_discover",
    "session_start",
    "session_before_switch",
    "session_before_fork",
    "session_before_compact",
    "session_shutdown",
    "before_provider_request",
    "after_provider_response",
    "input",
];

/// One choice for a property: left out, or this value.
pub type Class = Option<Value>;

/// The choices for a text property: missing, null, empty, and [`LITERAL`].
pub fn strings() -> Vec<Class> {
    vec![
        None,
        Some(Value::Null),
        Some(json!("")),
        Some(json!(LITERAL)),
    ]
}

/// The choices for a list of paths: missing, null, empty, and four entries with a duplicate.
pub fn lists() -> Vec<Class> {
    vec![
        None,
        Some(Value::Null),
        Some(json!([])),
        Some(json!(["z", "a", "z", LITERAL])),
    ]
}

/// The choices for a flag: missing, null, false and true.
pub fn flags() -> Vec<Class> {
    vec![
        None,
        Some(Value::Null),
        Some(json!(false)),
        Some(json!(true)),
    ]
}

/// The choices for a list of images: missing, null, empty, and two distinct images.
pub fn images() -> Vec<Class> {
    vec![
        None,
        Some(Value::Null),
        Some(json!([])),
        Some(json!([
            { "type": "image", "data": "AA==", "mimeType": "image/Ω" },
            { "type": "image", "data": "BB==", "mimeType": "image/png" },
        ])),
    ]
}

/// Every choice of one class for each of `N` properties; the first property varies slowest.
pub fn product<const N: usize>(classes: &[Class]) -> Vec<[Class; N]> {
    let mut rows: Vec<Vec<Class>> = vec![Vec::new()];
    for _ in 0..N {
        rows = rows
            .into_iter()
            .flat_map(|row| {
                classes.iter().map(move |class| {
                    let mut next = row.clone();
                    next.push(class.clone());
                    next
                })
            })
            .collect();
    }
    rows.into_iter()
        .filter_map(|row| <[Class; N]>::try_from(row).ok())
        .collect()
}

/// The document with the property `key` set to the class, or left out for a missing class.
pub fn with(mut document: Value, key: &str, class: &Class) -> Value {
    if let (Some(value), Some(object)) = (class, document.as_object_mut()) {
        object.insert(key.to_owned(), value.clone());
    }
    document
}

/// A valid document of the event kind `kind`.
pub fn document(kind: &str) -> Value {
    let message = json!({"role":"user","content":"first","timestamp":1.0});
    match kind {
        "message_update" => {
            json!({"type":kind,"message":message,"assistantMessageEvent":{"type":"start","partial":super::stream_events::assistant()}})
        }
        "tool_execution_start" => {
            json!({"type":kind,"toolCallId":"id","toolName":"tool","args":" opaque "})
        }
        "tool_execution_update" => {
            json!({"type":kind,"toolCallId":"id","toolName":"tool","args":" opaque ","partialResult":" partial "})
        }
        "tool_execution_end" => {
            json!({"type":kind,"toolCallId":"id","toolName":"tool","result":" final ","isError":false})
        }
        "model_select" => {
            json!({"type":kind,"model":super::selection_events::model(),"source":"set"})
        }
        "thinking_level_select" => json!({"type":kind,"level":"high","previousLevel":"minimal"}),
        "context" | "agent_end" => {
            json!({"type":kind,"messages":[message, {"role":"user","content":"second","timestamp":2.0}]})
        }
        "before_agent_start" => {
            json!({"type":kind,"prompt":"p","systemPrompt":"s","systemPromptOptions":{"cwd":"/work"}})
        }
        "agent_start" => json!({"type":kind}),
        "turn_start" => json!({"type":kind,"turnIndex":1.0,"timestamp":2.0}),
        "turn_end" => json!({"type":kind,"turnIndex":1.0,"message":message,"toolResults":[]}),
        "message_start" | "message_end" => json!({"type":kind,"message":message}),
        "resources_discover" => json!({ "type": kind, "cwd": "/work", "reason": "startup" }),
        "session_start" => json!({ "type": kind, "reason": "new" }),
        "session_shutdown" => json!({ "type": kind, "reason": "quit" }),
        "session_before_switch" => json!({ "type": kind, "reason": "resume" }),
        "session_before_fork" => json!({ "type": kind, "entryId": "e1", "position": "at" }),
        "session_before_compact" => json!({
            "type": kind,
            "branchEntries": [],
            "preparation": { "firstKeptEntryId": "e2", "isSplitTurn": false, "tokensBefore": 1234.0, "messagesToSummarize": [], "turnPrefixMessages": [], "fileOps": {"read":[],"written":[],"edited":[]}, "settings":{"enabled":true,"reserveTokens":1.0,"keepRecentTokens":2.0} },
        }),
        "before_provider_request" => json!({ "type": kind, "payload": "{\"model\":\"m\"}" }),
        "after_provider_response" => json!({
            "type": kind,
            "status": 200.0,
            "headers": [["content-type", "text/plain"]],
        }),
        _ => json!({ "type": kind, "text": "hello", "source": "interactive" }),
    }
}

/// A directive that makes the probe handler return a result of the `family` contract.
pub fn returns(family: &str, value: &Value) -> Value {
    json!({ "ending": { "returns": { "family": family, "value": value } } })
}

/// A directive that makes the probe handler fail with `message`.
pub fn fails(message: &str) -> Value {
    json!({ "ending": { "fails": message } })
}

/// The directive with the handler also editing its event in place.
pub fn marking(mut directive: Value) -> Value {
    directive["mark"] = json!(true);
    directive
}

/// The directive with the handler also replacing its event with the event of `document`.
pub fn replacing(mut directive: Value, document: Value) -> Value {
    directive["replaceWith"] = document;
    directive
}

/// How an event handler ended, as documents.
#[derive(Debug, PartialEq)]
pub enum Ended {
    /// It returned this result document, or none.
    Returned(Option<Value>),
    /// It failed with this message.
    Failed(String),
}

/// An event after its handler ran, and how the handler ended.
#[derive(Debug, PartialEq)]
pub struct Answer {
    /// The event as the handler left it, or none after a replacement by another kind.
    pub event: Option<Value>,
    /// How the handler ended.
    pub ended: Ended,
}

/// The document an encoded answer holds, if it holds one.
fn parsed(encoded: Encoded) -> Result<Option<Value>, String> {
    encoded?
        .map(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
        .transpose()
}

impl Answer {
    /// The answer of a handler that left `event` as it was and returned `result`.
    pub fn returned(event: &Value, result: Option<Value>) -> Self {
        Self {
            event: Some(event.clone()),
            ended: Ended::Returned(result),
        }
    }

    /// Reads a delivery.
    ///
    /// # Errors
    /// Returns the message when an encoded part is an encoding failure or not JSON.
    pub fn of(delivery: Delivery) -> Result<Self, String> {
        Ok(Self {
            event: parsed(delivery.event)?,
            ended: match delivery.decision {
                Decision::Returned(result) => Ended::Returned(parsed(result)?),
                Decision::Failed(message) => Ended::Failed(message),
            },
        })
    }
}

/// Delivers `event` to the probe handler with `directive` and the signal `signal`.
///
/// # Errors
/// Returns the message when the delivery or the reading of its answer fails.
pub async fn ask_with(
    driver: &mut impl Driver,
    event: &Value,
    directive: &Value,
    signal: Option<usize>,
) -> Result<Answer, String> {
    let handler = driver.identity("event probe")?;
    let delivery = driver
        .deliver(handler, &event.to_string(), &directive.to_string(), signal)
        .await?;
    Answer::of(delivery)
}

/// Delivers `event` to the probe handler with `directive`; a compaction is lent a signal that
/// is not cancelled.
///
/// # Errors
/// Returns the message when the delivery or the reading of its answer fails.
pub async fn ask(
    driver: &mut impl Driver,
    event: &Value,
    directive: &Value,
) -> Result<Answer, String> {
    let signal = if matches!(
        event["type"].as_str(),
        Some("session_before_compact" | "session_before_tree")
    ) {
        Some(driver.lend_signal(false)?)
    } else {
        None
    };
    ask_with(driver, event, directive, signal).await
}
