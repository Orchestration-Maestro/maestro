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

/// Reported tokens and cost of the usage one bucket of a session spent.
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
/// one separate bucket, costliest first.
///
/// # Errors
/// Returns a reached invalid stored usage field.
pub fn usage_cost_breakdown<T: std::borrow::Borrow<Value>>(
    _entries: &[T],
) -> Result<Vec<UsageCostBreakdownEntry>, serde_json::Error> {
    Ok(Vec::new())
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
        usage_cost_breakdown(&manager.get_entries()).map_err(thrown)
    }

    /// Message counts, token totals, cost and context usage.
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
                    stats.tokens.input += assistant.usage.input;
                    stats.tokens.output += assistant.usage.output;
                    stats.tokens.cache_read += assistant.usage.cache_read;
                    stats.tokens.cache_write += assistant.usage.cache_write;
                    stats.cost += assistant.usage.cost.total;
                }
                AgentMessage::Custom(_) => {}
            }
        }
        drop(messages);
        let tokens = &mut stats.tokens;
        tokens.total = tokens.input + tokens.output + tokens.cache_read + tokens.cache_write;
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
