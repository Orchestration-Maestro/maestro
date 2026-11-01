//! Preparation of discarded history, split prefixes and carried file operations.
use super::{CompactionPreparation, CompactionSettings, FileOperations};
use crate::session_manager::context::{generated_message, property};
use crate::{Presence, SessionEntry};
use maestro_json::is_truthy;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
/// Copy the messages the entries put in context, except a compaction entry's own summary.
fn messages(entries: &[Value]) -> Result<Vec<Value>, serde_json::Error> {
    let mut messages = Vec::new();
    for entry in entries {
        let message = match property(Some(entry), "type")?.and_then(Value::as_str) {
            Some("message") => entry
                .get("message")
                .filter(|value| is_truthy(value))
                .cloned(),
            Some("compaction") => None,
            _ => generated_message(entry),
        };
        if let Some(message) = message {
            messages.push(message);
        }
    }
    Ok(messages)
}
/// Carry built-in details from the preceding compaction.
fn prior_files(previous: Option<&Value>) -> Result<FileOperations, serde_json::Error> {
    let mut ops = super::create_file_ops();
    if let Some(previous) = previous {
        super::utils::carry_file_ops(previous, &mut ops)?;
    }
    Ok(ops)
}
/// Select the retained boundary and preserve previous-summary omission.
fn boundary(
    entries: &[Value],
    previous: Option<&Value>,
) -> Result<(usize, Presence<String>), serde_json::Error> {
    let Some(previous) = previous else {
        return Ok((0, Presence::Missing));
    };
    let summary = match previous.get("summary") {
        None => Presence::Missing,
        Some(Value::Null) => Presence::Null,
        Some(value) => Presence::Present(String::deserialize(value)?),
    };
    for (index, entry) in entries.iter().enumerate() {
        if property(Some(entry), "id")? == previous.get("firstKeptEntryId") {
            return Ok((index, summary));
        }
    }
    let index = entries
        .iter()
        .position(|entry| std::ptr::eq(entry, previous))
        .map_or(0, |index| index + 1);
    Ok((index, summary))
}
/// Prepare summary input without a model request, session mutation or persistence.
///
/// Returns absent for a last compaction or a missing usable first-kept identifier.
/// Enabled/reserve policy does not gate this manual preparation operation.
///
/// # Errors
/// Returns the native JSON error from reached context, estimate or stored detail fields.
pub fn prepare_compaction(
    path_entries: &[SessionEntry],
    settings: &CompactionSettings,
) -> Result<Option<CompactionPreparation>, serde_json::Error> {
    if let Some(last) = path_entries.last()
        && property(Some(last), "type")?.and_then(Value::as_str) == Some("compaction")
    {
        return Ok(None);
    }
    let previous = crate::get_latest_compaction_entry(path_entries)?;
    let (start, previous_summary) = boundary(path_entries, previous)?;
    let context =
        crate::build_session_context(path_entries, Presence::Missing, None::<&HashMap<_, _>>)?;
    let tokens_before = super::estimate_context_tokens(&context.messages)?.tokens;
    let cut = super::find_cut_point(
        path_entries,
        start,
        path_entries.len(),
        settings.keep_recent_tokens,
    )?;
    let Some(id) = path_entries
        .get(cut.first_kept_entry_index)
        .and_then(|entry| entry.get("id"))
        .and_then(Value::as_str)
        .filter(|id| !id.is_empty())
    else {
        return Ok(None);
    };
    let history_end = cut
        .turn_start_index
        .filter(|_| cut.is_split_turn)
        .unwrap_or(cut.first_kept_entry_index);
    let messages_to_summarize = messages(&path_entries[start..history_end])?;
    let turn_prefix_messages = if cut.is_split_turn {
        messages(&path_entries[history_end..cut.first_kept_entry_index])?
    } else {
        Vec::new()
    };
    let mut file_ops = prior_files(previous)?;
    for message in messages_to_summarize.iter().chain(&turn_prefix_messages) {
        super::extract_file_ops_from_message(message, &mut file_ops);
    }
    Ok(Some(CompactionPreparation {
        first_kept_entry_id: id.to_owned(),
        messages_to_summarize,
        turn_prefix_messages,
        is_split_turn: cut.is_split_turn,
        tokens_before,
        previous_summary,
        file_ops,
        settings: settings.clone(),
    }))
}
