//! Projection of caller-supplied entries without session-file reads or writes.
//! Stored timestamp conversion uses the existing [`crate::messages`] owner.
use super::types::{SessionContext, SessionContextModel, SessionEntry};
pub(super) use maestro_json::is_truthy as truthy;
use maestro_request::presence::Presence;
use serde::de::Error as _;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;

/// Read a property, failing only when its receiver is null or missing.
pub(crate) fn property<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<Option<&'a Value>, serde_json::Error> {
    match value {
        None | Some(Value::Null) => Err(serde_json::Error::custom(format!("cannot read {field}"))),
        Some(value) => Ok(value.get(field)),
    }
}

/// Collect a reachable ancestry, stopping before revisiting the same entry.
pub(crate) fn ancestor_path<'a>(
    leaf: &'a SessionEntry,
    lookup: impl Fn(&str) -> Option<&'a SessionEntry>,
) -> Result<Vec<&'a SessionEntry>, serde_json::Error> {
    let mut path = Vec::new();
    let mut visited = HashSet::new();
    let mut current = Some(leaf);
    while let Some(entry) = current {
        property(Some(entry), "id")?;
        if !visited.insert(std::ptr::from_ref(entry)) {
            break;
        }
        path.push(entry);
        current = entry
            .get("parentId")
            .and_then(Value::as_str)
            .filter(|id| !id.is_empty())
            .and_then(&lookup);
    }
    path.reverse();
    Ok(path)
}

/// Project a selected ancestry into owned raw context values.
///
/// Explicit null selects before-first after index construction. A supplied index,
/// including an empty one, is authoritative. Missing or unknown leaves use the last entry.
///
/// # Examples
/// ```
/// use maestro_request::presence::Presence;
/// use maestro_session::{build_session_context, parse_session_entries};
/// use std::collections::HashMap;
/// let entries = parse_session_entries(r#"{"type":"message","id":"a","parentId":null,"message":{"role":"user","content":"hello","timestamp":1}}"#);
/// let context = build_session_context(&entries, Presence::Missing, None::<&HashMap<_, _>>)?;
/// assert_eq!(context.messages[0]["content"], "hello");
/// # Ok::<(), serde_json::Error>(())
/// ```
///
/// # Errors
/// Returns a native JSON error at a reached null or missing property receiver.
pub fn build_session_context<'a, S: BuildHasher>(
    entries: &'a [SessionEntry],
    leaf_id: Presence<&str>,
    by_id: Option<&HashMap<String, &'a SessionEntry, S>>,
) -> Result<SessionContext, serde_json::Error> {
    if let Some(index) = by_id {
        return indexed_context(entries.last(), leaf_id, |id| index.get(id).copied());
    }
    let mut index = HashMap::new();
    for entry in entries {
        let id = property(Some(entry), "id")?
            .and_then(Value::as_str)
            .unwrap_or_default();
        index.insert(id.to_owned(), entry);
    }
    indexed_context(entries.last(), leaf_id, |id| index.get(id).copied())
}
/// Select and project against the caller's authoritative index.
pub(crate) fn indexed_context<'a>(
    last_entry: Option<&'a SessionEntry>,
    leaf_id: Presence<&str>,
    lookup: impl Fn(&str) -> Option<&'a SessionEntry>,
) -> Result<SessionContext, serde_json::Error> {
    let mut context = SessionContext {
        messages: Vec::new(),
        thinking_level: Presence::Present("off".into()),
        model: None,
    };
    let leaf = match leaf_id {
        Presence::Null => return Ok(context),
        Presence::Present(id) if !id.is_empty() => lookup(id),
        _ => None,
    }
    .or(last_entry);
    let Some(leaf) = leaf else {
        return Ok(context);
    };
    let path = ancestor_path(leaf, lookup)?;
    let compaction = scan_settings(&path, &mut context)?;
    context.messages = project_messages(&path, compaction);
    Ok(context)
}

/// Capture omission separately from an explicit null.
pub(super) fn observation(entry: &Value, field: &str) -> Presence<Value> {
    match entry.get(field) {
        None => Presence::Missing,
        Some(Value::Null) => Presence::Null,
        Some(value) => Presence::Present(value.clone()),
    }
}
/// Capture the surviving model fields without decoding them.
fn model(entry: &Value, model_field: &str) -> SessionContextModel {
    SessionContextModel {
        provider: observation(entry, "provider"),
        model_id: observation(entry, model_field),
    }
}

/// Scan settings before message emission, returning the latest path compaction.
fn scan_settings(
    path: &[&Value],
    context: &mut SessionContext,
) -> Result<Option<usize>, serde_json::Error> {
    let mut compaction = None;
    for (position, entry) in path.iter().enumerate() {
        match entry.get("type").and_then(Value::as_str) {
            Some("thinking_level_change") => {
                context.thinking_level = observation(entry, "thinkingLevel");
            }
            Some("model_change") => context.model = Some(model(entry, "modelId")),
            Some("message") => {
                if property(entry.get("message"), "role")?.and_then(Value::as_str)
                    == Some("assistant")
                {
                    context.model = entry.get("message").map(|message| model(message, "model"));
                }
            }
            Some("compaction") => compaction = Some(position),
            _ => {}
        }
    }
    Ok(compaction)
}
/// Select the eligible kept prefix and post-compaction messages.
fn project_messages(path: &[&Value], compaction: Option<usize>) -> Vec<Value> {
    let mut messages = Vec::new();
    if let Some(position) = compaction {
        let entry = path[position];
        messages.push(generated(
            entry,
            "compactionSummary",
            &["summary", "tokensBefore"],
        ));
        let cut = entry.get("firstKeptEntryId");
        if let Some(start) = path[..position]
            .iter()
            .position(|entry| entry.get("id") == cut)
        {
            messages.extend(
                path[start..position]
                    .iter()
                    .filter_map(|entry| append_message(entry)),
            );
        }
        messages.extend(
            path[position + 1..]
                .iter()
                .filter_map(|entry| append_message(entry)),
        );
    } else {
        messages.extend(path.iter().filter_map(|entry| append_message(entry)));
    }
    messages
}
/// Project an eligible stored message role.
fn append_message(entry: &Value) -> Option<Value> {
    match entry.get("type").and_then(Value::as_str) {
        Some("message") => entry.get("message").cloned(),
        Some("custom_message") => Some(generated(
            entry,
            "custom",
            &["customType", "content", "display", "details"],
        )),
        Some("branch_summary") if entry.get("summary").is_some_and(truthy) => {
            Some(generated(entry, "branchSummary", &["summary", "fromId"]))
        }
        _ => None,
    }
}
/// Capture generated message operands and convert their timestamp at projection.
pub(crate) fn generated(entry: &Value, role: &str, fields: &[&str]) -> Value {
    let mut message = serde_json::Map::new();
    message.insert("role".into(), role.into());
    for field in fields {
        if let Some(value) = entry.get(field) {
            message.insert((*field).into(), value.clone());
        }
    }
    let timestamp = entry
        .get("timestamp")
        .and_then(Value::as_str)
        .map_or(f64::NAN, crate::messages::timestamp_millis);
    message.insert("timestamp".into(), Value::from(timestamp));
    Value::Object(message)
}

/// Parse independent JSONL lines without validating a header or entry schema.
///
/// Outer ECMAScript whitespace is removed; remaining nonblank lines use native
/// JSON acceptance and limits. Malformed lines are skipped; empty JSON values are kept.
#[must_use]
pub fn parse_session_entries(content: &str) -> Vec<super::types::FileEntry> {
    let mut entries = Vec::new();
    // Reading a byte slice cannot fail, so the result holds no error to report.
    let _ = super::records::for_each_entry(
        content.as_bytes(),
        super::records::Admission::All,
        |entry| {
            entries.push(entry);
            std::ops::ControlFlow::Continue(())
        },
    );
    entries
}
pub(super) use maestro_text::is_whitespace_scalar as ecma_whitespace;

/// Borrow the last compaction in supplied order, independently of ancestry.
///
/// # Errors
/// Returns a native JSON error if reverse scanning reaches a null entry.
pub fn get_latest_compaction_entry<T: std::borrow::Borrow<SessionEntry>>(
    entries: &[T],
) -> Result<Option<&SessionEntry>, serde_json::Error> {
    for entry in entries.iter().rev() {
        let entry = entry.borrow();
        if property(Some(entry), "type")?.and_then(Value::as_str) == Some("compaction") {
            return Ok(Some(entry));
        }
    }
    Ok(None)
}
