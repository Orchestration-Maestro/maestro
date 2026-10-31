//! Session statistics, context usage and the last answer's text.
use super::{
    AgentSession, SessionStats, SessionStatsTokens,
    state::{message_records, thrown},
};
use crate::utils::lock::read;
use maestro_agent::AgentMessage;
use maestro_extensions::{ContextUsage, ExtensionExecution};
use maestro_models::{AssistantContent, DiagnosticErrorInfo, StopReason};
use maestro_session::{
    estimate_context_tokens, get_last_assistant_usage, get_latest_compaction_entry,
    session_manager::ReadonlySessionManager,
};
use serde_json::Value;

/// The bucket that holds usage no answering model owns: tools and summary requests.
const TOOLS_AND_SUMMARIES: &str = "Tools/summaries";

/// A stored numeric field: absent or non-numeric poisons the total, null counts as zero.
fn reported(record: &Value, field: &str) -> f64 {
    match record.get(field) {
        Some(Value::Number(number)) => number.as_f64().unwrap_or(f64::NAN),
        Some(Value::Null) => 0.0,
        Some(_) | None => f64::NAN,
    }
}

/// The failure for a stored usage record that is not the expected object.
fn record_error(field: &str) -> DiagnosticErrorInfo {
    DiagnosticErrorInfo::new(
        Some("TypeError".to_owned()),
        format!("the stored {field} is missing or not a number"),
    )
}

/// Tokens by kind and cost, added up over stored usage records.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UsageTotals {
    /// Reported input tokens.
    pub input: f64,
    /// Reported output tokens.
    pub output: f64,
    /// Reported cache reads.
    pub cache_read: f64,
    /// Reported cache writes.
    pub cache_write: f64,
    /// Reported cost.
    pub cost: f64,
}

impl UsageTotals {
    /// The totals before any usage contributed.
    const EMPTY: Self = Self {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        cost: 0.0,
    };

    /// Adds one stored usage record.
    ///
    /// # Errors
    /// Returns a record that is not an object, or whose cost is not.
    fn add(&mut self, usage: &Value) -> Result<(), DiagnosticErrorInfo> {
        if !usage.is_object() {
            return Err(record_error("usage"));
        }
        let cost = usage
            .get("cost")
            .filter(|cost| cost.is_object())
            .ok_or_else(|| record_error("usage.cost"))?;
        self.input += reported(usage, "input");
        self.output += reported(usage, "output");
        self.cache_read += reported(usage, "cacheRead");
        self.cache_write += reported(usage, "cacheWrite");
        self.cost += reported(cost, "total");
        Ok(())
    }

    /// Input, output, cache-read and cache-write tokens together.
    #[must_use]
    fn tokens(&self) -> f64 {
        self.input + self.output + self.cache_read + self.cache_write
    }
}

/// The bucket and usage record an entry reports, if it reports any: an answer under the model
/// that gave it, a tool result, compaction or branch summary under the shared bucket.
fn entry_usage(entry: &Value) -> Result<Option<(String, &Value)>, DiagnosticErrorInfo> {
    let message = entry.get("message").unwrap_or(&Value::Null);
    let truthy = |value| Option::filter(value, |value: &&Value| maestro_json::is_truthy(value));
    Ok(match entry.get("type").and_then(Value::as_str) {
        Some("message") => match message.get("role").and_then(Value::as_str) {
            Some("assistant") => {
                let name = |field| {
                    message
                        .get(field)
                        .and_then(Value::as_str)
                        .unwrap_or("undefined")
                };
                let model = message
                    .get("responseModel")
                    .filter(|model| maestro_json::is_truthy(model))
                    .and_then(Value::as_str)
                    .unwrap_or_else(|| name("model"));
                let usage = message.get("usage").ok_or_else(|| record_error("usage"))?;
                Some((format!("{}/{model}", name("provider")), usage))
            }
            Some("toolResult") => {
                truthy(message.get("usage")).map(|usage| (TOOLS_AND_SUMMARIES.to_owned(), usage))
            }
            _ => None,
        },
        Some("compaction" | "branch_summary") => {
            truthy(entry.get("usage")).map(|usage| (TOOLS_AND_SUMMARIES.to_owned(), usage))
        }
        _ => None,
    })
}

/// Everything a session reported: answers, tool results, compactions and branch summaries.
///
/// # Errors
/// Returns a reported usage record that is not an object with an object cost.
pub fn usage_totals<T: std::borrow::Borrow<Value>>(
    entries: &[T],
) -> Result<UsageTotals, DiagnosticErrorInfo> {
    let mut totals = UsageTotals::EMPTY;
    for entry in entries {
        if let Some((_, usage)) = entry_usage(entry.borrow())? {
            totals.add(usage)?;
        }
    }
    Ok(totals)
}

/// Reported tokens and cost of one bucket of a session's usage.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageCostBreakdownEntry {
    /// The provider and model that answered, or the shared bucket of tool and summary usage.
    pub key: String,
    /// Reported cost.
    pub cost: f64,
    /// Input, output, cache-read and cache-write tokens.
    pub tokens: f64,
}

/// Group a session's reported usage by answering model, keeping tool and summary usage in
/// one separate bucket, costliest first; buckets that reported nothing are left out.
///
/// # Errors
/// Returns a reported usage record that is not an object with an object cost.
pub fn usage_cost_breakdown<T: std::borrow::Borrow<Value>>(
    entries: &[T],
) -> Result<Vec<UsageCostBreakdownEntry>, DiagnosticErrorInfo> {
    let mut buckets: indexmap::IndexMap<String, UsageTotals> = indexmap::IndexMap::new();
    for entry in entries {
        if let Some((key, usage)) = entry_usage(entry.borrow())? {
            buckets
                .entry(key)
                .or_insert(UsageTotals::EMPTY)
                .add(usage)?;
        }
    }
    let mut breakdown: Vec<_> = buckets
        .into_iter()
        .map(|(key, totals)| UsageCostBreakdownEntry {
            key,
            cost: totals.cost,
            tokens: totals.tokens(),
        })
        .filter(|bucket| bucket.cost > 0.0 || bucket.tokens > 0.0)
        .collect();
    breakdown.sort_by(|a, b| b.cost.total_cmp(&a.cost));
    Ok(breakdown)
}

impl<E: ExtensionExecution> AgentSession<E> {
    /// The session's reported cost and tokens by answering model, with tool and summary
    /// usage in a bucket of its own.
    ///
    /// # Errors
    /// Returns a reached invalid stored usage field.
    pub fn get_usage_cost_breakdown(
        &self,
    ) -> Result<Vec<UsageCostBreakdownEntry>, DiagnosticErrorInfo> {
        let manager = self.session_manager.borrow();
        usage_cost_breakdown(&manager.get_entries())
    }

    /// Message counts of the agent's history; token and cost totals of everything the stored
    /// session reported, tool and summary usage included; and context usage.
    ///
    /// # Errors
    /// Returns a reached invalid stored field.
    pub fn get_session_stats(&self) -> Result<SessionStats, DiagnosticErrorInfo> {
        let messages = self.messages();
        let messages = read(&messages);
        let mut stats = SessionStats {
            session_file: self.session_file(),
            session_id: self.session_id(),
            user_messages: 0,
            assistant_messages: 0,
            tool_calls: 0,
            tool_results: 0,
            total_messages: messages.len(),
            tokens: SessionStatsTokens::default(),
            cost: 0.0,
            context_usage: None,
        };
        for message in messages.iter() {
            match message {
                AgentMessage::User(_) => stats.user_messages += 1,
                AgentMessage::ToolResult(_) => stats.tool_results += 1,
                AgentMessage::Assistant(assistant) => {
                    let assistant = read(assistant);
                    stats.assistant_messages += 1;
                    stats.tool_calls += assistant
                        .content
                        .iter()
                        .filter(|block| matches!(block, AssistantContent::ToolCall(_)))
                        .count();
                }
                AgentMessage::Custom(_) => {}
            }
        }
        drop(messages);
        let totals = usage_totals(&self.session_manager.borrow().get_entries())?;
        stats.tokens = SessionStatsTokens {
            input: totals.input,
            output: totals.output,
            cache_read: totals.cache_read,
            cache_write: totals.cache_write,
            total: totals.tokens(),
        };
        stats.cost = totals.cost;
        stats.context_usage = self.get_context_usage()?;
        Ok(stats)
    }

    /// Estimated context tokens against the model's window; unknown tokens after a compaction
    /// until an answer reports usage; absent without a window.
    ///
    /// # Errors
    /// Returns a reached invalid stored field.
    pub fn get_context_usage(&self) -> Result<Option<ContextUsage>, DiagnosticErrorInfo> {
        let context_window = read(&self.model()).context_window;
        if context_window <= 0.0 {
            return Ok(None);
        }
        if !self.has_usage_since_compaction()? {
            return Ok(Some(ContextUsage {
                tokens: None,
                context_window,
                percent: None,
            }));
        }
        let records = message_records(&read(&self.messages()))?;
        let tokens = estimate_context_tokens(&records).map_err(thrown)?.tokens;
        Ok(Some(ContextUsage {
            tokens: Some(tokens),
            context_window,
            percent: Some(tokens / context_window * 100.0),
        }))
    }

    /// Whether the branch has no compaction, or an answer after the latest one reports
    /// context tokens; aborted, failed and all-zero answers are skipped.
    fn has_usage_since_compaction(&self) -> Result<bool, DiagnosticErrorInfo> {
        let manager = self.session_manager.borrow();
        let branch = manager.get_branch(None).map_err(thrown)?;
        let Some(latest) = get_latest_compaction_entry(&branch).map_err(thrown)? else {
            return Ok(true);
        };
        let start = branch
            .iter()
            .rposition(|entry| std::ptr::eq(*entry, latest))
            .map_or(0, |index| index + 1);
        Ok(get_last_assistant_usage(&branch[start..])
            .map_err(thrown)?
            .is_some())
    }

    /// The trimmed text of the last answer, skipping an aborted empty one.
    #[must_use]
    pub fn get_last_assistant_text(&self) -> Option<String> {
        let messages = self.messages();
        let messages = read(&messages);
        let text = messages.iter().rev().find_map(|message| {
            let AgentMessage::Assistant(assistant) = message else {
                return None;
            };
            let assistant = read(assistant);
            if assistant.stop_reason == StopReason::Aborted && assistant.content.is_empty() {
                return None;
            }
            Some(
                assistant
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        AssistantContent::Text(text) => Some(text.text.as_str()),
                        _ => None,
                    })
                    .collect::<String>(),
            )
        })?;
        let text = text.trim_matches(maestro_text::is_whitespace_scalar);
        (!text.is_empty()).then(|| text.to_owned())
    }
}
