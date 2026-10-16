//! Raw exported document loading.
use super::{Stats, compute_stats};
use indexmap::IndexMap;
use maestro_app::{Presence, export_html::template::read_message, is_truthy};
use serde_json::{Map, Value};
use std::io;

/// Owned raw export data.
///
/// The loader prepares the location indexes and statistics once. Public edits
/// do not rebuild these snapshots.
#[derive(Debug)]
pub struct SessionDocument {
    /// Complete export object, including unknown fields.
    pub data: Map<String, Value>,
    /// Selected query leaf or the unchanged default, including omission.
    pub leaf_id: Presence<Value>,
    /// First decoded target parameter, including present-empty.
    pub url_target_id: Option<String>,
    /// Last record position per string identity, in first-seen identity order.
    pub by_id: IndexMap<String, usize>,
    /// Last assistant call location per string identity.
    pub tool_call_map: IndexMap<String, (usize, usize)>,
    /// Last truthy viewer label record per nonempty string target.
    pub label_map: IndexMap<String, usize>,
    /// Preparation-time accounting over all branches.
    pub global_stats: Stats,
}

/// Decode an exported base64 UTF-8 JSON object.
/// # Errors
/// Returns invalid data for invalid base64, JSON or document structure.
pub fn load_document(
    base64: &str,
    query: &str,
    injected_params: Option<&str>,
) -> io::Result<SessionDocument> {
    let data = decode(base64)?;
    let entries = data
        .get("entries")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid("missing entries array"))?;
    if entries.iter().any(Value::is_null) {
        return Err(invalid("null entry"));
    }
    let search = injected_params.unwrap_or_else(|| query.strip_prefix('?').unwrap_or(query));
    let mut params =
        url::form_urlencoded::parse(search.strip_prefix('?').unwrap_or(search).as_bytes());
    let selected = params
        .clone()
        .find(|(name, _)| name == "leafId")
        .map(|(_, value)| value.into_owned())
        .filter(|value| !value.is_empty());
    let leaf_id = match selected
        .map(Value::String)
        .or_else(|| data.get("leafId").cloned())
    {
        None => Presence::Missing,
        Some(Value::Null) => Presence::Null,
        Some(value) => Presence::Present(value),
    };
    let url_target_id = params
        .find(|(name, _)| name == "targetId")
        .map(|(_, value)| value.into_owned());
    let mut document = SessionDocument {
        data,
        leaf_id,
        url_target_id,
        global_stats: Stats::default(),
        by_id: IndexMap::new(),
        tool_call_map: IndexMap::new(),
        label_map: IndexMap::new(),
    };
    document.index()?;
    document.global_stats = compute_stats(
        document.data["entries"]
            .as_array()
            .ok_or_else(|| invalid("missing entries array"))?,
    )?;
    Ok(document)
}

/// Construct a native document admission failure.
pub(super) fn invalid(message: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

impl SessionDocument {
    /// Borrow a forest over the current raw entries with viewer-selected labels.
    #[must_use]
    pub fn build_tree(&self) -> Vec<maestro_app::SessionTreeNode<'_>> {
        let entries = self
            .data
            .get("entries")
            .and_then(Value::as_array)
            .map_or(&[][..], Vec::as_slice);
        maestro_app::export_html::template::build_tree(
            entries,
            |id| {
                self.label_map
                    .get(id)
                    .and_then(|&position| entries.get(position))
                    .and_then(|entry| entry.get("label"))
            },
            |_| None,
        )
    }

    /// Index assistant array calls only when their receiver is reached.
    fn index_calls(
        calls: &mut IndexMap<String, (usize, usize)>,
        entry: &Value,
        position: usize,
    ) -> io::Result<()> {
        if entry["type"] != "message" {
            return Ok(());
        }
        let message = read_message(entry)?;
        let Some(content) = message["content"]
            .as_array()
            .filter(|_| message["role"] == "assistant")
        else {
            return Ok(());
        };
        for (offset, block) in content.iter().enumerate() {
            let tool_call = is_tool_call(block)?;
            if let Some(id) = block["id"].as_str().filter(|_| tool_call) {
                calls.insert(id.to_owned(), (position, offset));
            }
        }
        Ok(())
    }

    /// Prepare positional lookups without copying raw histories.
    fn index(&mut self) -> io::Result<()> {
        let entries = self.data["entries"]
            .as_array()
            .ok_or_else(|| invalid("missing entries array"))?;
        for (position, entry) in entries.iter().enumerate() {
            if let Some(id) = entry["id"].as_str() {
                self.by_id.insert(id.to_owned(), position);
            }
            Self::index_calls(&mut self.tool_call_map, entry, position)?;
            if let Some(target) = entry["targetId"].as_str().filter(|target| {
                !target.is_empty() && entry["type"] == "label" && is_truthy(&entry["label"])
            }) {
                self.label_map.insert(target.to_owned(), position);
            }
        }
        Ok(())
    }
}

/// Frame standard base64, then reuse the shared whole-buffer UTF-8 decoder.
fn decode(base64: &str) -> io::Result<Map<String, Value>> {
    let bytes = crate::utils::base64::decode(base64)?;
    let data: Map<String, Value> =
        serde_json::from_str(&maestro_app::decode_utf8(&bytes)).map_err(invalid)?;
    Ok(data)
}

/// Read a reached content block's discriminator without narrowing its raw fields.
pub(super) fn is_tool_call(block: &Value) -> io::Result<bool> {
    if block.is_null() {
        return Err(invalid("null content block"));
    }
    Ok(block["type"] == "toolCall")
}
