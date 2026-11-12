//! Adds one property to a serialized object without building a JSON tree first.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use serde::Serialize;

/// Serializes `value` straight to text and appends `"key": property` as its last member.
///
/// The text equals what parsing into a tree, inserting the property and printing it gives, as
/// long as `value` has no member named `key` of its own.
///
/// # Errors
/// Returns the serialization failure, or a message when `value` is not a JSON object.
pub(crate) fn with_property(
    value: &impl Serialize,
    key: &str,
    property: &(impl Serialize + ?Sized),
) -> Result<String, String> {
    let text = serde_json::to_string(value).map_err(|error| error.to_string())?;
    let Some(members) = text
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return Err("value is not a JSON object".to_owned());
    };
    let key = serde_json::to_string(key).map_err(|error| error.to_string())?;
    let property = serde_json::to_string(property).map_err(|error| error.to_string())?;
    let separator = if members.is_empty() { "" } else { "," };
    Ok(format!("{{{members}{separator}{key}:{property}}}"))
}

#[cfg(test)]
mod tests;
