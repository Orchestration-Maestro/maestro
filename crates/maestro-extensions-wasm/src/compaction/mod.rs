//! Compaction data authors read and supply.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::{Deserialize, Serialize};

use crate::types::Presence;

/// What a compaction is about to summarize; only the fields delivered so far.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompactionPreparation {
    /// The first entry the compaction keeps.
    pub first_kept_entry_id: String,
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
