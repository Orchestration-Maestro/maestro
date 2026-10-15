//! Complete original-history JSONL download preparation.
use super::{SessionDocument, document::invalid};
use crate::browser::attachment_content::{AttachmentHost, AttachmentUiError};
use maestro_app::{BorrowedJson, compact_json, compact_members, is_truthy};
use serde_json::Value;
use std::io;

/// Download the raw header followed by every stored entry, independent of the selected leaf.
///
/// # Errors
/// Returns reached invalid metadata, serialization or supplied host failures.
pub fn download_session_json(
    document: &SessionDocument,
    host: &dyn AttachmentHost,
) -> Result<(), AttachmentUiError> {
    let header = document.data.get("header").filter(|value| is_truthy(value));
    let header = header
        .map(|value| value.as_object().ok_or_else(|| invalid("nonobject header")))
        .transpose()?;
    let entries = document
        .data
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing entries array"))?;
    let mut lines = Vec::new();
    if let Some(header) = header {
        let fallback = Value::String("header".into());
        let first = (
            "type",
            BorrowedJson::Value(header.get("type").unwrap_or(&fallback)),
        );
        let rest = header
            .iter()
            .filter(|(name, _)| name.as_str() != "type")
            .map(|(name, value)| (name.as_str(), BorrowedJson::Value(value)));
        lines.push(compact_members(std::iter::once(first).chain(rest)).map_err(io::Error::other)?);
    }
    for entry in entries {
        lines.push(compact_json(entry).map_err(io::Error::other)?);
    }
    let id = header
        .and_then(|header| header.get("id"))
        .filter(|value| is_truthy(value));
    let name = match id {
        None => "session",
        Some(Value::String(value)) => value,
        Some(_) => return Err(invalid("nonstring session identity").into()),
    };
    host.download_bytes(
        lines.join("\n").as_bytes(),
        &format!("{name}.jsonl"),
        "application/x-ndjson",
    )
}
