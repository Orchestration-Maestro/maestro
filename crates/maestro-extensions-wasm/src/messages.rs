//! Application-owned conversation messages.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use crate::{Presence, UserContent};
use serde::{Deserialize, Serialize};

/// Recorded shell execution output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "bashExecution", rename_all = "camelCase")]
pub struct BashExecutionMessage {
    /// Command that ran.
    pub command: String,
    /// Captured output.
    pub output: String,
    /// Supplied exit code.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub exit_code: Presence<f64>,
    /// Whether execution was cancelled.
    pub cancelled: bool,
    /// Whether output was truncated.
    pub truncated: bool,
    /// Path to complete output, when supplied.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub full_output_path: Presence<String>,
    /// Recorded time.
    pub timestamp: f64,
    /// Whether the message is excluded from model context.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub exclude_from_context: Presence<bool>,
}

/// An extension-injected conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "custom", rename_all = "camelCase")]
pub struct CustomMessage {
    /// Extension-defined classification.
    pub custom_type: String,
    /// Text or text/image blocks.
    pub content: UserContent,
    /// Whether the message is displayed.
    pub display: bool,
    /// Opaque details text.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<String>,
    /// Recorded time.
    pub timestamp: f64,
}

/// A summary of a conversation branch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "branchSummary", rename_all = "camelCase")]
pub struct BranchSummaryMessage {
    /// Summary text.
    pub summary: String,
    /// Entry the branch came from.
    pub from_id: String,
    /// Recorded time.
    pub timestamp: f64,
}

/// A summary retained after compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename = "compactionSummary", rename_all = "camelCase")]
pub struct CompactionSummaryMessage {
    /// Summary text.
    pub summary: String,
    /// Supplied token count before compaction.
    pub tokens_before: f64,
    /// Recorded time.
    pub timestamp: f64,
}
