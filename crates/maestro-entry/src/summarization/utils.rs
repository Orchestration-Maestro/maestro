//! File-operation sets shared by compaction and branch summarization.
use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

/// Insertion-ordered sets of authored file paths, by operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperations {
    /// Paths read.
    pub read: IndexSet<String>,
    /// Paths written.
    pub written: IndexSet<String>,
    /// Paths edited.
    pub edited: IndexSet<String>,
}
