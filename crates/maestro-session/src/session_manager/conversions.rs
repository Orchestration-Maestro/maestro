//! The two historical session-record conversions.
use super::{FileEntry, generate_id};
use serde_json::{Value, json};
use std::{collections::HashSet, io};
use uuid::Uuid;

/// Apply historical ID/parent and custom-message conversions in place.
///
/// # Errors
/// Returns invalid data for a reached null record or nonnumeric version.
pub fn migrate_session_entries(entries: &mut [FileEntry]) -> io::Result<()> {
    migrate(entries, Uuid::new_v4).map(|_| ())
}

/// Convert selected versions, reporting whether rewriting is required.
pub(super) fn migrate(
    entries: &mut [FileEntry],
    mut next_uuid: impl FnMut() -> Uuid,
) -> io::Result<bool> {
    let header = entries.iter().find(|entry| entry["type"] == "session");
    let version = match header.and_then(|header| header.get("version")) {
        None | Some(Value::Null) => 1.0,
        Some(value) => value
            .as_f64()
            .ok_or_else(|| invalid("nonnumeric version"))?,
    };
    if version >= 3.0 {
        return Ok(false);
    }
    if version < 2.0 {
        assign_ids(entries, &mut next_uuid)?;
        resolve_cuts(entries);
    }
    for entry in entries {
        if entry.is_null() {
            return Err(invalid("null session record"));
        }
        if entry["type"] == "session" {
            entry["version"] = json!(3);
        } else if entry["type"] == "message" && entry["message"]["role"] == "hookMessage" {
            entry["message"]["role"] = json!("custom");
        }
    }
    Ok(true)
}

/// Allocate final identities before resolving cuts that can point forward.
fn assign_ids(entries: &mut [FileEntry], next_uuid: &mut impl FnMut() -> Uuid) -> io::Result<()> {
    let mut ids = HashSet::new();
    let mut previous = Value::Null;
    for entry in entries {
        if entry["type"] == "session" {
            entry["version"] = json!(2);
            continue;
        }
        let id = generate_id(|id| ids.contains(id), &mut *next_uuid);
        ids.insert(id.clone());
        let object = entry
            .as_object_mut()
            .ok_or_else(|| invalid("nonobject session record"))?;
        object.insert("id".into(), json!(id));
        object.insert("parentId".into(), previous);
        previous = json!(id);
    }
    Ok(())
}

/// Resolve numeric header-inclusive cuts against the final identity sequence.
fn resolve_cuts(entries: &mut [FileEntry]) {
    for position in 0..entries.len() {
        let entry = &entries[position];
        if entry["type"] != "compaction" {
            continue;
        }
        let Some(index) = entry["firstKeptEntryIndex"].as_f64() else {
            continue;
        };
        let spelling = index.to_string();
        let target = spelling
            .strip_prefix("-0")
            .filter(|suffix| suffix.is_empty())
            .map_or(spelling.as_str(), |_| "0")
            .parse::<usize>()
            .ok()
            .and_then(|index| entries.get(index))
            .filter(|entry| entry["type"] != "session")
            .map(|entry| entry["id"].clone());
        let entry = &mut entries[position];
        if let Some(target) = target {
            entry["firstKeptEntryId"] = target;
        }
        if let Some(object) = entry.as_object_mut() {
            object.shift_remove("firstKeptEntryIndex");
        }
    }
}

/// Construct a native selected-field error.
pub(super) fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
