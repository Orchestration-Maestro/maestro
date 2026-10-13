//! Content and part unions and the selected part records.

use serde_json::{Map, Value, json};

use super::{Failure, Projection, Step, malformed, record, record_list, type_name};

/// Selection of a content: its parts, then its role.
const CONTENT: &[Step] = &[
    Step::Convert("parts", |parts| record_list(parts, PART)),
    Step::Keep("role"),
];

/// Selection of a part, in wire order; a part may hold several members at once.
const PART: &[Step] = &[
    Step::Keep("mediaResolution"),
    Step::Keep("codeExecutionResult"),
    Step::Keep("executableCode"),
    Step::Convert("fileData", |data| record(data, FILE_DATA)),
    Step::Convert("functionCall", |call| record(call, FUNCTION_CALL)),
    Step::Keep("functionResponse"),
    Step::Convert("inlineData", |blob| record(blob, BLOB)),
    Step::Keep("text"),
    Step::Keep("thought"),
    Step::Keep("thoughtSignature"),
    Step::Keep("videoMetadata"),
    Step::Keep("toolCall"),
    Step::Keep("toolResponse"),
    Step::Keep("partMetadata"),
];

/// Selection of file data.
const FILE_DATA: &[Step] = &[
    Step::Reject("displayName"),
    Step::Keep("fileUri"),
    Step::Keep("mimeType"),
];

/// Selection of a function call.
const FUNCTION_CALL: &[Step] = &[
    Step::Keep("id"),
    Step::Keep("args"),
    Step::Keep("name"),
    Step::Reject("partialArgs"),
    Step::Reject("willContinue"),
];

/// Selection of inline data.
const BLOB: &[Step] = &[
    Step::Keep("data"),
    Step::Reject("displayName"),
    Step::Keep("mimeType"),
];

/// Whether the value is a content: an object holding a part list.
fn is_content(value: &Value) -> bool {
    value.get("parts").is_some_and(Value::is_array)
}

/// Whether the value is a part that must sit inside a content.
fn is_call(value: &Value) -> bool {
    value.as_object().is_some_and(|part| {
        part.contains_key("functionCall") || part.contains_key("functionResponse")
    })
}

/// One part from a part union: objects are kept and text becomes a text part.
fn part(origin: Value) -> Projection<Value> {
    match origin {
        Value::Null => Err(Failure::error("PartUnion is required")),
        Value::String(text) => Ok(json!({ "text": text })),
        Value::Object(_) | Value::Array(_) => Ok(origin),
        other => Err(Failure::error(format!(
            "Unsupported part type: {}",
            type_name(&other)
        ))),
    }
}

/// The parts of a part-list union; an empty list is rejected.
fn parts(origin: Value) -> Projection<Vec<Value>> {
    match origin {
        Value::Array(items) if items.is_empty() => Err(Failure::error("PartListUnion is required")),
        Value::Array(items) => items.into_iter().map(part).collect(),
        single => Ok(vec![part(single)?]),
    }
}

/// Keep a content or wrap an admitted part-list union in a user turn.
fn content(origin: Value) -> Projection<Value> {
    if is_content(&origin) {
        return Ok(origin);
    }
    Ok(user_turn(parts(origin)?))
}

/// A user content holding the parts.
fn user_turn(parts: Vec<Value>) -> Value {
    let mut turn = Map::new();
    turn.insert("role".to_owned(), json!("user"));
    turn.insert("parts".to_owned(), Value::Array(parts));
    Value::Object(turn)
}

/// The wire form of a system instruction.
pub(super) fn system_instruction(origin: Value) -> Projection<Value> {
    record(content(origin)?, CONTENT)
}

/// The wire form of the request contents: one content, one part, or a homogeneous list.
pub(super) fn contents(origin: Value) -> Projection<Value> {
    let turns = match origin {
        Value::Array(items) if items.is_empty() => {
            return Err(Failure::error("contents are required"));
        }
        Value::Array(items) => turn_list(items)?,
        single if is_call(&single) => {
            return Err(Failure::error(
                "To specify functionCall or functionResponse parts, please wrap them in a Content object, specifying the role for them",
            ));
        }
        single => vec![content(single)?],
    };
    record_list(Value::Array(turns), CONTENT)
}

/// Keep a non-empty content list or group admitted non-call parts into one user turn.
fn turn_list(items: Vec<Value>) -> Projection<Vec<Value>> {
    let of_contents = is_content(&items[0]);
    let mut turns = Vec::new();
    let mut loose = Vec::new();
    for item in items {
        if is_content(&item) != of_contents {
            return Err(malformed(
                "Mixing Content and Parts is not supported, please group the parts into a the appropriate Content objects and specify the roles for them",
            ));
        }
        if of_contents {
            turns.push(item);
        } else if is_call(&item) {
            return Err(malformed(
                "To specify functionCall or functionResponse parts, please wrap them, and any other parts, in Content objects as appropriate, specifying the role for them",
            ));
        } else {
            loose.push(item);
        }
    }
    if !of_contents {
        turns.push(user_turn(parts(Value::Array(loose))?));
    }
    Ok(turns)
}
