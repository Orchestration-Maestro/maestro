//! Compaction data authors read and supply.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::{Deserialize, Serialize};

use crate::AgentMessage;
use crate::types::Presence;
mod utils;
pub use utils::FileOperations;

/// Supplied messages, files and settings for a pending compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionPreparation {
    /// The first entry the compaction keeps.
    pub first_kept_entry_id: String,
    /// Messages selected for the summary.
    pub messages_to_summarize: Vec<AgentMessage>,
    /// Messages preceding the retained portion of a split turn.
    pub turn_prefix_messages: Vec<AgentMessage>,
    /// Authored paths grouped by operation.
    pub file_ops: FileOperations,
    /// Supplied compaction settings.
    pub settings: CompactionSettings,
    /// Whether the compaction cuts through a turn.
    pub is_split_turn: bool,
    /// The context size before the compaction.
    pub tokens_before: f64,
    /// The summary of an earlier compaction, when there is one.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub previous_summary: Presence<String>,
}

/// A compaction an extension supplies.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResult {
    /// The authored summary.
    pub summary: String,
    /// The first entry the compaction keeps.
    pub first_kept_entry_id: String,
    /// The context size before the compaction.
    pub tokens_before: f64,
    /// Opaque JSON text, carried without parsing or reformatting.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub details: Presence<String>,
}

/// Settings supplied with a prepared compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionSettings {
    /// Whether compaction is enabled.
    pub enabled: bool,
    /// Tokens reserved for a response.
    pub reserve_tokens: f64,
    /// Recent tokens to retain.
    pub keep_recent_tokens: f64,
}
