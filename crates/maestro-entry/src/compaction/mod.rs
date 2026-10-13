//! Compaction records an extension reads and supplies, generic over the message view.
use maestro_request::presence::Presence;
use serde::{Deserialize, Serialize};
mod utils;
pub use utils::FileOperations;

/// Messages, file operations and settings prepared for a pending compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionPreparation<M> {
    /// First entry the compaction keeps.
    pub first_kept_entry_id: String,
    /// Messages to summarize.
    pub messages_to_summarize: Vec<M>,
    /// Messages before the kept part of a split turn.
    pub turn_prefix_messages: Vec<M>,
    /// Whether the cut falls inside a turn.
    pub is_split_turn: bool,
    /// Context size before compaction.
    pub tokens_before: f64,
    /// Summary of the previous compaction, if any.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub previous_summary: Presence<String>,
    /// Files touched by the summarized messages.
    pub file_ops: FileOperations,
    /// Compaction settings in effect.
    pub settings: CompactionSettings,
}

/// Compaction settings with every value supplied.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionSettings {
    /// Whether compaction is enabled.
    pub enabled: bool,
    /// Tokens reserved for a response.
    pub reserve_tokens: f64,
    /// Recent tokens to keep.
    pub keep_recent_tokens: f64,
}
