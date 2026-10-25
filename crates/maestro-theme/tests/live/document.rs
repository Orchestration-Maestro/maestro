//! Custom documents derived from the shipped dark theme.
use serde_json::Value;

/// The shipped dark theme document.
const DARK: &str = include_str!("../../assets/theme/dark.json");

/// The shipped dark document under another name with a literal accent.
pub fn custom_json(name: &str, accent: &str) -> Value {
    let mut json: Value = serde_json::from_str(DARK).unwrap();
    json["name"] = name.into();
    json["colors"]["accent"] = accent.into();
    json
}
