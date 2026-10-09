//! JSON text as ECMAScript `JSON.stringify` writes it, and the reading of numbers and objects
//! that `JSON.parse` gives: every number is the double it rounds to.

use indexmap::IndexMap;
use num_traits::ToPrimitive;
use serde::de::DeserializeOwned;
use serde::de::value::MapDeserializer;
use serde_json::value::RawValue;
use serde_json::{Map, Number, Value};

/// Serialize without whitespace, keys in canonical order, numbers as ECMAScript prints them.
///
/// Keys that are canonical array indices come first in ascending numeric order, then
/// every other key in insertion order. A number is written as the double it names, so an
/// integer beyond 2^53 is rounded.
///
/// # Errors
/// Returns the serializer failure when a string cannot be written.
pub(crate) fn compact_json(value: &Value) -> Result<String, serde_json::Error> {
    let mut text = String::new();
    write_value(value, &mut text)?;
    Ok(text)
}

/// Serialize an object exactly as [`compact_json`] serializes a value holding it.
///
/// # Errors
/// Returns the serializer failure when a string cannot be written.
pub(crate) fn compact_object(members: &Map<String, Value>) -> Result<String, serde_json::Error> {
    let mut text = String::new();
    write_object(members, &mut text)?;
    Ok(text)
}

/// Append one value to the output text.
fn write_value(value: &Value, text: &mut String) -> Result<(), serde_json::Error> {
    match value {
        Value::Null => text.push_str("null"),
        Value::Bool(flag) => text.push_str(if *flag { "true" } else { "false" }),
        Value::Number(number) => write_number(number, text),
        Value::String(string) => text.push_str(&serde_json::to_string(string)?),
        Value::Array(items) => {
            text.push('[');
            for (position, item) in items.iter().enumerate() {
                if position > 0 {
                    text.push(',');
                }
                write_value(item, text)?;
            }
            text.push(']');
        }
        Value::Object(members) => write_object(members, text)?,
    }
    Ok(())
}

/// Append a number in ECMAScript spelling.
fn write_number(number: &Number, text: &mut String) {
    let float = number.as_f64().unwrap_or_default();
    text.push_str(ryu_js::Buffer::new().format_finite(float));
}

/// Append an object with array-index keys first.
fn write_object(members: &Map<String, Value>, text: &mut String) -> Result<(), serde_json::Error> {
    let mut ordered: Vec<(&String, &Value)> = members.iter().collect();
    ordered.sort_by_key(|(key, _)| array_index(key).unwrap_or(u32::MAX));
    text.push('{');
    for (position, (key, value)) in ordered.into_iter().enumerate() {
        if position > 0 {
            text.push(',');
        }
        text.push_str(&serde_json::to_string(key)?);
        text.push(':');
        write_value(value, text)?;
    }
    text.push('}');
    Ok(())
}

/// Parse a canonical array index: no sign, no leading zero, below 2^32 - 1.
fn array_index(key: &str) -> Option<u32> {
    let canonical =
        key == "0" || (!key.starts_with('0') && key.bytes().all(|b| b.is_ascii_digit()));
    canonical
        .then(|| key.parse::<u32>().ok())
        .flatten()
        .filter(|index| *index != u32::MAX)
}

/// Containers a value may nest when [`json_value`] decodes it: the most that the parser's
/// recursion limit of 128 accepts.
pub(crate) const MAX_NESTING: usize = 127;

/// Read external JSON text as a raw value. The text may nest to any depth: skipping over
/// nested containers takes no recursion.
///
/// # Errors
/// Returns the parser failure for malformed text.
pub(crate) fn raw_json(text: &str) -> Result<&RawValue, serde_json::Error> {
    serde_json::from_str(text)
}

/// The members of a JSON object in order; a repeated name keeps its first position and last value.
type Members<'a> = IndexMap<String, &'a RawValue>;

/// Read a JSON number as the double it rounds to: beyond the range of a double it is an
/// infinity, below it a zero. Anything else, including a number-like string, is `None`.
pub(crate) fn raw_number(raw: &RawValue) -> Option<f64> {
    raw.get().parse().ok()
}

/// Read an object as a record whose repeated member names keep their last value. A value that
/// is not an object, arrays included, and a record the type rejects are `None`.
pub(crate) fn object_record<T: DeserializeOwned>(raw: &RawValue) -> Option<T> {
    let members: Members<'_> = serde_json::from_str(raw.get()).ok()?;
    T::deserialize(MapDeserializer::<_, serde_json::Error>::new(
        members.into_iter(),
    ))
    .ok()
}

/// The value of an object member, the last one when the name repeats; `None` when `raw` is not
/// an object or lacks the member.
pub(crate) fn member<'a>(raw: &'a RawValue, name: &str) -> Option<&'a RawValue> {
    let members: Members<'a> = serde_json::from_str(raw.get()).ok()?;
    members.get(name).copied()
}

/// Report whether a value counts as present when used as a condition.
///
/// Null, `false`, zero of either sign and the empty string do not; arrays, objects and
/// infinities always do.
pub(crate) fn is_truthy(raw: &RawValue) -> bool {
    match raw.get() {
        "null" | "false" | r#""""# => false,
        _ => raw_number(raw) != Some(0.0),
    }
}

/// Decode a value into the JSON data model, where every number is a double and an infinity,
/// which JSON cannot spell, becomes `null`. Decoding recurses once per level, so a value that
/// nests more than [`MAX_NESTING`] containers fails as malformed text does.
/// Exactly representable integral doubles use integer storage where in range, except
/// negative zero, whose sign is retained. Compact number spelling is unchanged.
///
/// # Errors
/// Returns the decoder failure for malformed text, or a recursion-limit failure for a value
/// nested too deeply.
pub(crate) fn json_value(raw: &RawValue) -> Result<Value, serde_json::Error> {
    decode(raw, MAX_NESTING)
}

/// Decode a value that may hold `containers` more levels of containers.
fn decode(raw: &RawValue, containers: usize) -> Result<Value, serde_json::Error> {
    if let Some(number) = raw_number(raw) {
        return Ok(project_number(number));
    }
    match raw.get().as_bytes().first() {
        Some(b'[' | b'{') if containers == 0 => {
            Err(serde::de::Error::custom("recursion limit exceeded"))
        }
        Some(b'[') => serde_json::from_str::<Vec<&RawValue>>(raw.get())?
            .into_iter()
            .map(|item| decode(item, containers - 1))
            .collect(),
        Some(b'{') => serde_json::from_str::<Members<'_>>(raw.get())?
            .into_iter()
            .map(|(key, member)| Ok((key, decode(member, containers - 1)?)))
            .collect(),
        _ => serde_json::from_str(raw.get()),
    }
}

/// Store integral doubles losslessly as integers, except negative zero whose sign is retained.
fn project_number(number: f64) -> Value {
    if number.is_finite() && number.fract() == 0.0 && !(number == 0.0 && number.is_sign_negative())
    {
        if let Some(integer) = number.to_i64() {
            return Value::from(integer);
        }
        if let Some(integer) = number.to_u64() {
            return Value::from(integer);
        }
    }
    Value::from(number)
}

/// Serialize a value like [`compact_json`] serializes its [`json_value`].
///
/// # Errors
/// Returns the failure of decoding, which includes excess nesting, or of writing.
pub(crate) fn compact_raw(raw: &RawValue) -> Result<String, serde_json::Error> {
    compact_json(&json_value(raw)?)
}

#[cfg(test)]
mod tests;
