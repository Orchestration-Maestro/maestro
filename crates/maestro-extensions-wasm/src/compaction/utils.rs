//! File-operation categories supplied to summarization.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use serde::{Deserialize, Serialize};
/// Insertion-ordered sets of authored file paths.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperations {
    /// Paths read.
    pub read: indexmap::IndexSet<String>,
    /// Paths written.
    pub written: indexmap::IndexSet<String>,
    /// Paths edited.
    pub edited: indexmap::IndexSet<String>,
}
