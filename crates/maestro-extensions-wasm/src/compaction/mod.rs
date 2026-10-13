//! Compaction data authors read and supply.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use serde::{Deserialize, Serialize};

use crate::AgentMessage;
use crate::types::Presence;
pub use maestro_entry::compaction::{CompactionSettings, FileOperations};

/// Supplied messages, files and settings for a pending compaction.
pub type CompactionPreparation = maestro_entry::compaction::CompactionPreparation<AgentMessage>;

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
