//! Tool record vocabulary.
use serde::{Deserialize, Serialize};

/// Tool selection limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TruncationBy {
    /// Newline-entry quota.
    Lines,
    /// UTF-8 byte quota.
    Bytes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Supplied truncation output and accounting.
pub struct TruncationResult<N> {
    /// Content.
    pub content: String,
    /// Truncated.
    pub truncated: bool,
    /// Truncated by.
    pub truncated_by: Option<TruncationBy>,
    /// Total lines.
    pub total_lines: N,
    /// Total bytes.
    pub total_bytes: N,
    /// Output lines.
    pub output_lines: N,
    /// Output bytes.
    pub output_bytes: N,
    /// Last line partial.
    pub last_line_partial: bool,
    /// First line exceeds limit.
    pub first_line_exceeds_limit: bool,
    /// Max lines.
    pub max_lines: N,
    /// Max bytes.
    pub max_bytes: N,
}
