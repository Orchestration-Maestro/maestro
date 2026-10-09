//! Selected raw members and diagnostic rendering.

use serde::de::{DeserializeOwned, Error};
use serde_json::value::RawValue;

use super::native;
use crate::DiagnosticErrorInfo;
use crate::providers::json_text::{compact_raw, is_truthy, member, raw_json, raw_number};

/// Decode a selected typed member; missing members are absent and invalid types fail.
pub(super) fn field<T: DeserializeOwned>(
    raw: &RawValue,
    name: &str,
) -> Result<Option<T>, DiagnosticErrorInfo> {
    let Some(value) = member(raw, name) else {
        return Ok(None);
    };
    serde_json::from_str(value.get())
        .map(Some)
        .map_err(|error| native(&error))
}

/// Require a value before a branch dereferences its properties.
pub(super) fn required(raw: Option<&RawValue>) -> Result<&RawValue, DiagnosticErrorInfo> {
    raw.filter(|raw| raw.get() != "null").ok_or_else(|| {
        native(&serde_json::Error::custom(
            "missing or null required container",
        ))
    })
}

/// Recognize string discriminators without coercing other values into a kind.
pub(super) fn kind(raw: &RawValue, name: &str) -> Result<String, DiagnosticErrorInfo> {
    match member(raw, name).filter(|value| value.get().starts_with('"')) {
        Some(value) => spelled(Some(value)),
        None => Ok(String::new()),
    }
}

/// Decode a required string at a typed assignment boundary.
pub(super) fn string(raw: Option<&RawValue>) -> Result<String, DiagnosticErrorInfo> {
    serde_json::from_str(raw.map_or("null", RawValue::get)).map_err(|error| native(&error))
}

/// Select truthy argument text, applying the empty default before typed decoding.
pub(super) fn arguments(raw: &RawValue) -> Result<String, DiagnosticErrorInfo> {
    member(raw, "arguments")
        .filter(|value| is_truthy(value))
        .map_or_else(|| Ok(String::new()), |value| string(Some(value)))
}

/// Select the last child when a guarded branch uses a truthy array container.
pub(super) fn last(raw: Option<&RawValue>) -> Result<Option<&RawValue>, DiagnosticErrorInfo> {
    let Some(raw) = raw.filter(|value| is_truthy(value)) else {
        return Ok(None);
    };
    let parts: Vec<&RawValue> = serde_json::from_str(raw.get()).map_err(|error| native(&error))?;
    Ok(parts.last().copied())
}

/// Retain only the newly appended last part, without decoding its fields.
pub(super) fn appended(
    part: Option<&RawValue>,
) -> Result<Option<Box<RawValue>>, DiagnosticErrorInfo> {
    part.map(|part| {
        RawValue::from_string(format!("[{}]", part.get())).map_err(|error| native(&error))
    })
    .transpose()
}

/// Join selected text fields of final parts.
pub(super) fn joined(
    raw: &RawValue,
    name: &str,
    message: bool,
) -> Result<String, DiagnosticErrorInfo> {
    let selected = member(raw, name);
    let parts: Vec<&RawValue> = if !message && selected.is_none_or(|raw| raw.get() == "null") {
        Vec::new()
    } else {
        serde_json::from_str(selected.map_or("null", RawValue::get))
            .map_err(|error| native(&error))?
    };
    parts
        .into_iter()
        .map(|part| {
            let part = required(Some(part))?;
            let key = if message && kind(part, "type")? != "output_text" {
                "refusal"
            } else {
                "text"
            };
            let value = member(part, key).filter(|raw| raw.get() != "null");
            value.map_or_else(|| Ok(String::new()), |raw| spelled(Some(raw)))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join(if message { "" } else { "\n\n" }))
}

/// Encode selected identity and truthy phase without applying replay recognition rules.
pub(super) fn signature(raw: &RawValue) -> Result<String, DiagnosticErrorInfo> {
    let mut payload = String::from("{\"v\":1");
    for key in ["id", "phase"] {
        if let Some(value) = member(raw, key)
            && (key == "id" || is_truthy(value))
        {
            payload.push_str(",\"");
            payload.push_str(key);
            payload.push_str("\":");
            payload.push_str(value.get());
        }
    }
    payload.push('}');
    compact_raw(raw_json(&payload).map_err(|error| native(&error))?).map_err(|error| native(&error))
}

/// Render diagnostic values, leaving object interiors opaque and expanding arrays iteratively.
pub(super) fn spelled(raw: Option<&RawValue>) -> Result<String, DiagnosticErrorInfo> {
    let Some(raw) = raw else {
        return Ok("undefined".to_owned());
    };
    let mut pending = vec![Some(raw)];
    let mut output = String::new();
    while let Some(next) = pending.pop() {
        let Some(value) = next else {
            output.push(',');
            continue;
        };
        match value.get().as_bytes().first() {
            Some(b'{') => output.push_str("[object Object]"),
            Some(b'[') => {
                let children: Vec<&RawValue> =
                    serde_json::from_str(value.get()).map_err(|error| native(&error))?;
                pending.extend(children.into_iter().enumerate().rev().flat_map(
                    |(index, child)| {
                        let value = (child.get() != "null").then_some(Some(child));
                        value.into_iter().chain((index > 0).then_some(None))
                    },
                ));
            }
            Some(b'"') => output.push_str(
                &serde_json::from_str::<String>(value.get()).map_err(|error| native(&error))?,
            ),
            _ => {
                if let Some(number) = raw_number(value) {
                    output.push_str(ryu_js::Buffer::new().format(number));
                } else {
                    output.push_str(value.get());
                }
            }
        }
    }
    Ok(output)
}
