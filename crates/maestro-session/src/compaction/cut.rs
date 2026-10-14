//! Retained-message cuts and turn boundaries in supplied entry ranges.
use crate::SessionEntry;
use crate::session_manager::context::property;
use serde_json::Value;
/// Read a message role only after reaching a message entry.
fn message_role(entry: &Value) -> Result<Option<&str>, serde_json::Error> {
    Ok(property(entry.get("message"), "role")?.and_then(Value::as_str))
}
/// Find the closest user, shell or generated-entry turn start within the lower bound.
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
        match property(Some(entry), "type")?.and_then(Value::as_str) {
            Some("branch_summary" | "custom_message") => return Ok(Some(index)),
            Some("message") if matches!(message_role(entry)?, Some("user" | "bashExecution")) => {
                return Ok(Some(index));
            }
            _ => {}
        }
    }
    Ok(None)
}
/// Select message and generated-entry candidates before estimating any tokens.
fn candidates(
    entries: &[Value],
    start: usize,
    end: usize,
) -> Result<Vec<usize>, serde_json::Error> {
    let mut cuts = Vec::new();
    for (index, entry) in entries.iter().enumerate().take(end).skip(start) {
        let valid = match property(Some(entry), "type")?.and_then(Value::as_str) {
            Some("branch_summary" | "custom_message") => true,
            Some("message") => matches!(
                message_role(entry)?,
                Some(
                    "user"
                        | "assistant"
                        | "bashExecution"
                        | "custom"
                        | "branchSummary"
                        | "compactionSummary"
                )
            ),
            _ => false,
        };
        if valid {
            cuts.push(index);
        }
    }
    Ok(cuts)
}
/// Find the budget crossing while counting only stored message entries.
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
        if property(Some(entry), "type")?.and_then(Value::as_str) != Some("message") {
            continue;
        }
        tokens += super::estimate_tokens(entry.get("message").unwrap_or(&Value::Null))?;
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
        match property(entries.get(cut - 1), "type")?.and_then(Value::as_str) {
            Some("message" | "compaction") => break,
            _ => cut -= 1,
        }
    }
    let entry = &entries[cut];
    let user = entry.get("type").and_then(Value::as_str) == Some("message")
        && message_role(entry)? == Some("user");
    let turn_start_index = if user {
        None
    } else {
        find_turn_start_index(entries, cut, start_index)?
    };
    Ok(super::CutPointResult {
        first_kept_entry_index: cut,
        turn_start_index,
        is_split_turn: !user && turn_start_index.is_some(),
    })
}
