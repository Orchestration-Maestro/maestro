//! Supplied truncate tool data.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
/// The limit reported as causing truncation.
pub enum TruncationLimit {
    /// Lines payload.
    Lines,
    /// Bytes payload.
    Bytes,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Supplied truncation output and accounting.
pub struct TruncationResult {
    /// Content.
    pub content: String,
    /// Truncated.
    pub truncated: bool,
    /// Truncated by.
    pub truncated_by: Option<TruncationLimit>,
    /// Total lines.
    pub total_lines: f64,
    /// Total bytes.
    pub total_bytes: f64,
    /// Output lines.
    pub output_lines: f64,
    /// Output bytes.
    pub output_bytes: f64,
    /// Last line partial.
    pub last_line_partial: bool,
    /// First line exceeds limit.
    pub first_line_exceeds_limit: bool,
    /// Max lines.
    pub max_lines: f64,
    /// Max bytes.
    pub max_bytes: f64,
}
