//! Custom documents derived from the shipped dark theme.
use serde_json::Value;

/// The shipped dark theme document.
const DARK: &str = include_str!("../../assets/theme/dark.json");

/// The shipped dark document under another name with a literal accent.
///
/// # Errors
/// Returns an error if the bundled dark theme document is invalid JSON.
pub fn custom_json(name: &str, accent: &str) -> Result<Value, serde_json::Error> {
    let mut json: Value = serde_json::from_str(DARK)?;
    json["name"] = name.into();
    json["colors"]["accent"] = accent.into();
    Ok(json)
}
