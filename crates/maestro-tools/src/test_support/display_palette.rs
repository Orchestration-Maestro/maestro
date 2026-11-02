//! Recorded display palette.
use serde::Deserialize;

/// The committed palette of the display recording.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Palette {
    /// Theme keys, whose index plus one is each key's palette color.
    theme_keys: Vec<String>,
}

/// Return the recorded palette keys in their recorded order.
///
/// # Errors
/// Returns failures from the supplied values or collaborators.
pub fn palette_keys(source: &str) -> Result<Vec<String>, serde_json::Error> {
    Ok(serde_json::from_str::<Palette>(source)?.theme_keys)
}
