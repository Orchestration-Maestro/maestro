//! Selected reads over raw JSON members, shared by the protocol reducers.

use serde::de::{DeserializeOwned, Error};
use serde_json::value::RawValue;

use crate::DiagnosticErrorInfo;
use crate::providers::json_text::{member, or_zero, raw_number};

/// A failure that keeps the message of the JSON reader that raised it.
pub(crate) fn native(error: &serde_json::Error) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo {
        name: None,
        message: error.to_string(),
        stack: None,
        code: None,
    }
}

/// Read a numeric usage member, defaulting only absent, null and zero values to zero.
pub(crate) fn count(raw: &RawValue, name: &str) -> Result<f64, DiagnosticErrorInfo> {
    let Some(value) = member(raw, name).filter(|value| value.get() != "null") else {
        return Ok(0.0);
    };
    raw_number(value)
        .map(|number| or_zero(Some(number)))
        .ok_or_else(|| native(&serde_json::Error::custom("expected a numeric usage count")))
}

/// Decode a selected typed member; missing members are absent and invalid types fail.
pub(crate) fn field<T: DeserializeOwned>(
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
pub(crate) fn required(raw: Option<&RawValue>) -> Result<&RawValue, DiagnosticErrorInfo> {
    raw.filter(|raw| raw.get() != "null").ok_or_else(|| {
        native(&serde_json::Error::custom(
            "missing or null required container",
        ))
    })
}

/// Decode a required string at its selected read site.
pub(crate) fn string(raw: Option<&RawValue>) -> Result<String, DiagnosticErrorInfo> {
    serde_json::from_str(raw.map_or("null", RawValue::get)).map_err(|error| native(&error))
}
