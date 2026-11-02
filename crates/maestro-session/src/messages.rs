//! Formatting and projection of application conversation messages.
use chrono::{DateTime, Local, NaiveDate, NaiveDateTime, TimeZone};
use maestro_agent::AgentMessage;
use maestro_models::{Message, TextContent, UserBlock, UserContent, UserMessage};
pub use maestro_request::messages::{
    BashExecutionMessage, BranchSummaryMessage, CompactionSummaryMessage, CustomMessage,
    CustomMessageInput, MessageDelivery, MessageRenderOptions, SendMessageOptions,
    SendUserMessageOptions, UserMessageDelivery,
};
use maestro_request::presence::Presence;
use std::convert::Infallible;

/// Format a recorded shell execution for model input.
#[must_use]
pub fn bash_execution_to_text(message: &BashExecutionMessage) -> String {
    shell_text(&ShellText {
        command: &message.command,
        output: &message.output,
        cancelled: message.cancelled,
        truncated: message.truncated,
        exit_code: message.exit_code.present().copied(),
        full_output_path: message.full_output_path.present().map(String::as_str),
    })
}
/// Consumed shell fields shared by typed and historical projections.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ShellText<'a> {
    /// Executed command.
    command: &'a str,
    /// Captured output.
    output: &'a str,
    /// Supplied status.
    exit_code: Option<f64>,
    /// Whether execution was cancelled.
    #[serde(default)]
    cancelled: bool,
    /// Whether output was clipped.
    #[serde(default)]
    truncated: bool,
    /// Path to the full output.
    full_output_path: Option<&'a str>,
}
/// Format the selected shell fields, without reading execution metadata.
fn shell_text(message: &ShellText<'_>) -> String {
    let mut text = format!("Ran `{}`\n", message.command);
    if message.output.is_empty() {
        text.push_str("(no output)");
    } else {
        text.push_str("```\n");
        text.push_str(message.output);
        text.push_str("\n```");
    }
    if message.cancelled {
        text.push_str("\n\n(command cancelled)");
    } else if let Some(code) = message.exit_code
        && code != 0.0
    {
        text.push_str("\n\nCommand exited with code ");
        text.push_str(ryu_js::Buffer::new().format(code));
    }
    if message.truncated
        && let Some(path) = message.full_output_path
        && !path.is_empty()
    {
        text.push_str("\n\n[Output truncated. Full output: ");
        text.push_str(path);
        text.push(']');
    }
    text
}

/// Retain a branch summary with its parsed timestamp.
#[must_use]
pub fn create_branch_summary_message(
    summary: String,
    from_id: String,
    timestamp: &str,
) -> BranchSummaryMessage {
    BranchSummaryMessage {
        summary,
        from_id,
        timestamp: timestamp_millis(timestamp),
    }
}

/// Retain a compaction summary with its parsed timestamp.
#[must_use]
pub fn create_compaction_summary_message(
    summary: String,
    tokens_before: f64,
    timestamp: &str,
) -> CompactionSummaryMessage {
    CompactionSummaryMessage {
        summary,
        tokens_before,
        timestamp: timestamp_millis(timestamp),
    }
}

/// Parse ISO instants, UTC calendar dates or local date-times into epoch milliseconds.
pub(crate) fn timestamp_millis(text: &str) -> f64 {
    parse_timestamp_millis(text).map_or(f64::NAN, millis_number)
}

/// Parses supported date text into epoch milliseconds, retaining invalid-date absence.
#[must_use]
pub fn parse_timestamp_millis(text: &str) -> Option<i64> {
    if let Ok(date) = DateTime::parse_from_rfc3339(text) {
        return Some(date.timestamp_millis());
    }
    if let Ok(date) = NaiveDate::parse_from_str(text, "%Y-%m-%d") {
        return date
            .and_hms_opt(0, 0, 0)
            .map(|time| time.and_utc().timestamp_millis());
    }
    NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .and_then(local_timestamp_millis)
}

/// Resolve a local date-time to epoch milliseconds.
///
/// A repeated local time selects the earlier instant. A skipped local time uses
/// the offset in force before the transition, so it resolves past the gap.
#[must_use]
pub fn local_timestamp_millis(value: NaiveDateTime) -> Option<i64> {
    match Local.from_local_datetime(&value) {
        chrono::LocalResult::Single(date) => Some(date.timestamp_millis()),
        chrono::LocalResult::Ambiguous(earlier, later) => {
            Some(earlier.timestamp_millis().min(later.timestamp_millis()))
        }
        chrono::LocalResult::None => {
            Some(value.and_utc().timestamp_millis() - gap_offset_millis(value))
        }
    }
}

/// The offset a zone applies before the transition that skips `reading`, in milliseconds.
///
/// The first probe reads the offset at the reading taken as UTC, the second the
/// offset one probe earlier. A forward transition makes the earlier one the
/// offset in force before it, so the smaller of the two applies.
fn gap_offset_millis(reading: NaiveDateTime) -> i64 {
    let first = Local.offset_from_utc_datetime(&reading);
    let first_seconds = i64::from(first.local_minus_utc());
    let second =
        Local.offset_from_utc_datetime(&(reading - chrono::Duration::seconds(first_seconds)));
    i64::from(second.local_minus_utc().min(first.local_minus_utc())) * 1_000
}

/// Retain injected content, visibility and details with a parsed timestamp.
pub fn create_custom_message<T>(
    custom_type: String,
    content: maestro_models::UserContent,
    display: bool,
    details: Presence<T>,
    timestamp: &str,
) -> CustomMessage<T> {
    CustomMessage {
        custom_type,
        content,
        display,
        details,
        timestamp: timestamp_millis(timestamp),
    }
}

/// Application message roles plus caller-defined entries.
#[derive(Clone)]
pub enum CustomAgentMessages<T = serde_json::Value, C = Infallible> {
    /// Recorded shell execution.
    BashExecution(BashExecutionMessage),
    /// Injected content.
    Custom(CustomMessage<T>),
    /// Branch summary.
    BranchSummary(BranchSummaryMessage),
    /// Compaction summary.
    CompactionSummary(CompactionSummaryMessage),
    /// Caller-defined role.
    Other(C),
}
impl<T, C: maestro_agent::CustomAgentMessages> maestro_agent::CustomAgentMessages
    for CustomAgentMessages<T, C>
{
    fn role(&self) -> &str {
        match self {
            Self::BashExecution(_) => "bashExecution",
            Self::Custom(_) => "custom",
            Self::BranchSummary(_) => "branchSummary",
            Self::CompactionSummary(_) => "compactionSummary",
            Self::Other(message) => message.role(),
        }
    }
}

impl<T: serde::Serialize> serde::Serialize for CustomAgentMessages<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::BashExecution(message) => message.serialize(serializer),
            Self::Custom(message) => message.serialize(serializer),
            Self::BranchSummary(message) => message.serialize(serializer),
            Self::CompactionSummary(message) => message.serialize(serializer),
            Self::Other(never) => match *never {},
        }
    }
}
/// Reads a stored record by its `role`; an unknown role is an error.
impl<'de, T: serde::de::DeserializeOwned> serde::Deserialize<'de> for CustomAgentMessages<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let value = serde_json::Value::deserialize(deserializer)?;
        let message = match value.get("role").and_then(serde_json::Value::as_str) {
            Some("bashExecution") => serde_json::from_value(value).map(Self::BashExecution),
            Some("custom") => serde_json::from_value(value).map(Self::Custom),
            Some("branchSummary") => serde_json::from_value(value).map(Self::BranchSummary),
            Some("compactionSummary") => serde_json::from_value(value).map(Self::CompactionSummary),
            role => {
                let role = role.unwrap_or_default();
                return Err(D::Error::custom(format!("unknown message role `{role}`")));
            }
        };
        message.map_err(D::Error::custom)
    }
}
/// Project application entries into an owned, ordered model transcript.
#[must_use]
pub fn convert_to_llm<T, C: maestro_agent::CustomAgentMessages>(
    messages: &[AgentMessage<CustomAgentMessages<T, C>>],
) -> Vec<Message> {
    messages
        .iter()
        .filter_map(|entry| match entry {
            AgentMessage::Custom(entry) => {
                let message = read(entry);
                project_custom(&message)
            }
            AgentMessage::User(entry) => Some(Message::User(read(entry).clone())),
            AgentMessage::Assistant(entry) => Some(Message::Assistant(read(entry).clone())),
            AgentMessage::ToolResult(entry) => Some(Message::ToolResult(read(entry).clone())),
        })
        .collect()
}

/// Convert plain text into one block, retaining existing block arrays.
fn user_message(content: UserContent, timestamp: f64) -> Message {
    let content = match content {
        UserContent::Text(text) => {
            UserContent::Blocks(vec![UserBlock::Text(TextContent::new(text))])
        }
        blocks @ UserContent::Blocks(_) => blocks,
    };
    Message::User(UserMessage { content, timestamp })
}

/// Opening wrapper for compaction summaries.
pub const COMPACTION_SUMMARY_PREFIX: &str = "The conversation history before this point was compacted into the following summary:\n\n<summary>\n";
/// Closing wrapper for compaction summaries.
pub const COMPACTION_SUMMARY_SUFFIX: &str = "\n</summary>";
/// Opening wrapper for branch summaries.
pub const BRANCH_SUMMARY_PREFIX: &str =
    "The following is a summary of a branch that this conversation came back from:\n\n<summary>\n";
/// Closing wrapper for branch summaries.
pub const BRANCH_SUMMARY_SUFFIX: &str = "</summary>";

/// Convert integer epoch milliseconds to the floating-point message representation.
pub(crate) fn millis_number(millis: i64) -> f64 {
    millis.to_string().parse().unwrap_or(f64::NAN)
}

/// Select model content from one application entry.
fn project_custom<T, C>(message: &CustomAgentMessages<T, C>) -> Option<Message> {
    match message {
        CustomAgentMessages::Custom(message) => {
            Some(user_message(message.content.clone(), message.timestamp))
        }
        CustomAgentMessages::BranchSummary(message) => Some(user_message(
            UserContent::Text(summary_wrapper(&message.summary, true)),
            message.timestamp,
        )),
        CustomAgentMessages::CompactionSummary(message) => Some(user_message(
            UserContent::Text(summary_wrapper(&message.summary, false)),
            message.timestamp,
        )),
        CustomAgentMessages::BashExecution(message)
            if !matches!(message.exclude_from_context, Presence::Present(true)) =>
        {
            Some(user_message(
                UserContent::Text(bash_execution_to_text(message)),
                message.timestamp,
            ))
        }
        CustomAgentMessages::BashExecution(_) | CustomAgentMessages::Other(_) => None,
    }
}

/// Borrow a retained entry, recovering its data after poisoning.
fn read<T>(entry: &std::sync::RwLock<T>) -> std::sync::RwLockReadGuard<'_, T> {
    entry
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Select only summary-visible fields from historical model messages.
pub(crate) fn project_summary(
    message: &serde_json::Value,
) -> Result<Option<crate::compaction::utils::SummaryMessage<'_>>, serde_json::Error> {
    use crate::compaction::utils::{SummaryBlock, SummaryMessage};
    use serde::Deserialize;
    let content = &message["content"];
    Ok(match message["role"].as_str() {
        Some("user" | "custom") => Some(SummaryMessage::User(match content.as_str() {
            Some(text) => std::borrow::Cow::Borrowed(text),
            None => std::borrow::Cow::Owned(summary_text(content)?),
        })),
        Some("toolResult") => Some(SummaryMessage::Tool(summary_text(content)?)),
        Some("assistant") => {
            let blocks = content.as_array().ok_or_else(|| {
                <serde_json::Error as serde::de::Error>::custom("expected content array")
            })?;
            let selected = blocks
                .iter()
                .filter_map(|block| match block["type"].as_str() {
                    Some("text") => {
                        Some(<&str>::deserialize(&block["text"]).map(SummaryBlock::Text))
                    }
                    Some("toolCall") => Some((|| {
                        Ok(SummaryBlock::Tool(
                            <&str>::deserialize(&block["name"])?,
                            &block["arguments"],
                        ))
                    })()),
                    _ => None,
                })
                .collect::<Result<Vec<_>, serde_json::Error>>()?;
            Some(SummaryMessage::Assistant(selected))
        }
        Some("branchSummary" | "compactionSummary") => Some(SummaryMessage::User(
            std::borrow::Cow::Owned(summary_wrapper(
                <&str>::deserialize(&message["summary"])?,
                message["role"] == "branchSummary",
            )),
        )),
        Some("bashExecution") if !maestro_json::is_truthy(&message["excludeFromContext"]) => {
            Some(SummaryMessage::User(std::borrow::Cow::Owned(shell_text(
                &ShellText::deserialize(message)?,
            ))))
        }
        _ => None,
    })
}
/// Concatenate only consumed text blocks, ignoring unknown content kinds.
fn summary_text(content: &serde_json::Value) -> Result<String, serde_json::Error> {
    use serde::Deserialize;
    content
        .as_array()
        .ok_or_else(|| <serde_json::Error as serde::de::Error>::custom("expected content array"))?
        .iter()
        .filter(|block| block["type"] == "text")
        .map(|block| <&str>::deserialize(&block["text"]))
        .collect()
}

/// Wrap retained summaries identically for model and summary input.
fn summary_wrapper(summary: &str, branch: bool) -> String {
    let (prefix, suffix) = if branch {
        (BRANCH_SUMMARY_PREFIX, BRANCH_SUMMARY_SUFFIX)
    } else {
        (COMPACTION_SUMMARY_PREFIX, COMPACTION_SUMMARY_SUFFIX)
    };
    format!("{prefix}{summary}{suffix}")
}
