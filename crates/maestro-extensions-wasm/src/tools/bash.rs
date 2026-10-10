//! Supplied bash tool data.
use super::truncate::TruncationResult;
use crate::Presence;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the bash tool.
pub struct BashToolInput {
    /// Command.
    pub command: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Timeout.
    pub timeout: Presence<f64>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Observations supplied by the bash tool.
pub struct BashToolDetails {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Truncation.
    pub truncation: Presence<TruncationResult>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Full output path.
    pub full_output_path: Presence<String>,
}
