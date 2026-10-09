//! Selected raw members and diagnostic rendering.

use serde::de::DeserializeOwned;
use serde_json::error::Category;
use serde_json::value::RawValue;

use super::native;
use crate::DiagnosticErrorInfo;
use crate::providers::json_text::{member, raw_number};

/// Decode only the named scalar; missing and wrong-typed values are absent.
pub(super) fn field<T: DeserializeOwned>(
    raw: &RawValue,
    name: &str,
) -> Result<Option<T>, DiagnosticErrorInfo> {
    let Some(value) = member(raw, name) else {
        return Ok(None);
    };
    match serde_json::from_str(value.get()) {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.classify() == Category::Data => Ok(None),
        Err(error) => Err(native(&error)),
    }
}

/// Read a selected string, defaulting absent values to empty text.
pub(super) fn text(raw: &RawValue, name: &str) -> Result<String, DiagnosticErrorInfo> {
    Ok(field(raw, name)?.unwrap_or_default())
}

/// Read array children without decoding their contents.
pub(super) fn array(raw: Option<&RawValue>) -> Vec<&RawValue> {
    raw.and_then(|raw| serde_json::from_str(raw.get()).ok())
        .unwrap_or_default()
}

/// Join selected text fields of final parts.
pub(super) fn joined(
    raw: &RawValue,
    name: &str,
    message: bool,
) -> Result<String, DiagnosticErrorInfo> {
    array(member(raw, name))
        .into_iter()
        .map(|part| {
            let key = if message && text(part, "type")? != "output_text" {
                "refusal"
            } else {
                "text"
            };
            text(part, key)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join(if message { "" } else { "\n\n" }))
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
                pending.extend(array(Some(value)).into_iter().enumerate().rev().flat_map(
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
