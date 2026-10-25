//! Canonical argument serialization and shared diagnostic path display.

use serde_json::{Map, Value};

/// Serialize original arguments with canonical keys and binary64 number spelling.
pub(super) fn pretty(value: &Value) -> Result<String, serde_json::Error> {
    maestro_json::pretty_json(value)
}

/// Enumerate numeric-index keys first while preserving other insertion order.
pub(super) fn entries(object: &Map<String, Value>) -> Vec<(&String, &Value)> {
    maestro_json::canonical_entries(object)
}

/// Display instance separators and append a literal nonempty missing name.
pub(super) fn path(instance: &str, missing: Option<&str>) -> String {
    let mut display = instance
        .strip_prefix('/')
        .unwrap_or(instance)
        .replace('/', ".");
    if let Some(name) = missing.filter(|name| !name.is_empty()) {
        if !display.is_empty() {
            display.push('.');
        }
        display.push_str(name);
    }
    if display.is_empty() {
        "root".into()
    } else {
        display
    }
}
