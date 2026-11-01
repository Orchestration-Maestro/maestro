//! Compaction data authors read and supply.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]

use crate::AgentMessage;
pub use maestro_entry::compaction::{CompactionSettings, FileOperations};

/// Supplied messages, files and settings for a pending compaction.
pub type CompactionPreparation = maestro_entry::compaction::CompactionPreparation<AgentMessage>;

/// A compaction an extension supplies, with opaque JSON text details.
pub type CompactionResult = maestro_entry::compaction::CompactionResult<crate::JsonText>;

/// Native transcript serialization.
pub mod utils;
