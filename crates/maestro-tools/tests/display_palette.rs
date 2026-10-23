//! Recorded display palette.
#![cfg(test)]
use serde::Deserialize;

/// The committed palette of the display recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Palette {
    /// Theme keys, whose index plus one is each key's palette color.
    theme_keys: Vec<String>,
}

/// Return the recorded palette keys in their recorded order.
pub fn palette_keys(source: &str) -> Vec<String> {
    serde_json::from_str::<Palette>(source).unwrap().theme_keys
}
