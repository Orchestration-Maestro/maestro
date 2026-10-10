//! Supplied read tool data.
use super::truncate::TruncationResult;
use crate::Presence;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the read tool.
pub struct ReadToolInput {
    /// Path.
    pub path: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Offset.
    pub offset: Presence<f64>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Limit.
    pub limit: Presence<f64>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Observations supplied by the read tool.
pub struct ReadToolDetails {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// Truncation.
    pub truncation: Presence<TruncationResult>,
}
