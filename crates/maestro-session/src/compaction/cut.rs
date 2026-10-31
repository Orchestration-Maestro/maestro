//! Retained-message cuts and turn boundaries in supplied entry ranges.
use crate::SessionEntry;
use crate::session_manager::context::{generated_message, property};
use serde_json::Value;
use std::borrow::Cow;
/// Roles whose messages can begin the retained span.
const CUT_ROLES: [&str; 6] = [
    "user",
    "assistant",
    "bashExecution",
    "custom",
    "branchSummary",
    "compactionSummary",
];
/// Roles that begin a turn: every cut role except an assistant answer.
const TURN_START_ROLES: [&str; 5] = [
    "user",
    "bashExecution",
    "custom",
    "branchSummary",
    "compactionSummary",
];
/// The message an entry puts in model context, a compaction entry's summary included.
///
/// A stored message is borrowed as written, so a missing message stays a reached null.
fn context_message(entry: &Value) -> Result<Option<Cow<'_, Value>>, serde_json::Error> {
    if property(Some(entry), "type")?.and_then(Value::as_str) == Some("message") {
        return Ok(Some(Cow::Borrowed(
            entry.get("message").unwrap_or(&Value::Null),
        )));
    }
    Ok(generated_message(entry).map(Cow::Owned))
}
/// Whether the entry puts a message with one of `roles` in model context.
///
/// A compaction entry only stands for the span it replaced, so it never qualifies.
fn has_role(entry: &Value, roles: &[&str]) -> Result<bool, serde_json::Error> {
    if entry.get("type").and_then(Value::as_str) == Some("compaction") {
        return Ok(false);
    }
    let Some(message) = context_message(entry)? else {
        return Ok(false);
    };
    Ok(property(Some(&message), "role")?
        .and_then(Value::as_str)
        .is_some_and(|role| roles.contains(&role)))
}
/// Find the closest context-visible turn start within the lower bound.
///
/// # Errors
/// Returns a native JSON error for a reached null entry or message.
pub fn find_turn_start_index(
    entries: &[SessionEntry],
    entry_index: usize,
    start_index: usize,
) -> Result<Option<usize>, serde_json::Error> {
    for index in (start_index..=entry_index).rev() {
        let Some(entry) = entries.get(index) else {
            continue;
        };
        if has_role(entry, &TURN_START_ROLES)? {
            return Ok(Some(index));
        }
    }
    Ok(None)
}
/// Select context-visible cut candidates before estimating any tokens.
fn candidates(
    entries: &[Value],
    start: usize,
    end: usize,
) -> Result<Vec<usize>, serde_json::Error> {
    let mut cuts = Vec::new();
    for (index, entry) in entries.iter().enumerate().take(end).skip(start) {
        if has_role(entry, &CUT_ROLES)? {
            cuts.push(index);
        }
    }
    Ok(cuts)
}
/// Find the budget crossing while counting every message the entries put in context.
fn crossing(
    entries: &[Value],
    start: usize,
    end: usize,
    keep: f64,
) -> Result<Option<usize>, serde_json::Error> {
    let mut tokens = 0.0;
    for index in (start..end).rev() {
        let Some(entry) = entries.get(index) else {
            continue;
        };
        let Some(message) = context_message(entry)? else {
            continue;
        };
        let weight = super::estimate_tokens(&message)?;
        tokens += weight;
        if tokens >= keep {
            return Ok(Some(index));
        }
    }
    Ok(None)
}
/// Select a safe cut in the supplied half-open range and include preceding metadata.
///
/// The caller supplies `start_index <= end_index <= entries.len()`.
///
/// # Errors
/// Returns a native JSON error for reached null entries or selected message fields.
pub fn find_cut_point(
    entries: &[SessionEntry],
    start_index: usize,
    end_index: usize,
    keep_recent_tokens: f64,
) -> Result<super::CutPointResult, serde_json::Error> {
    let cuts = candidates(entries, start_index, end_index)?;
    let Some(&first) = cuts.first() else {
        return Ok(super::CutPointResult {
            first_kept_entry_index: start_index,
            turn_start_index: None,
            is_split_turn: false,
        });
    };
    let mut cut = crossing(entries, start_index, end_index, keep_recent_tokens)?
        .and_then(|crossing| {
            cuts.iter()
                .copied()
                .find(|&candidate| candidate >= crossing)
        })
        .unwrap_or(first);
    while cut > start_index {
        let previous = &entries[cut - 1];
        if context_message(previous)?.is_some() {
            break;
        }
        cut -= 1;
    }
    let starts_turn = has_role(&entries[cut], &TURN_START_ROLES)?;
    let turn_start_index = if starts_turn {
        None
    } else {
        find_turn_start_index(entries, cut, start_index)?
    };
    Ok(super::CutPointResult {
        first_kept_entry_index: cut,
        turn_start_index,
        is_split_turn: !starts_turn && turn_start_index.is_some(),
    })
}
