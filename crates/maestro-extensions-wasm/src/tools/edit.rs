//! Supplied edit tool data.
use crate::Presence;
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the edit tool.
pub struct EditToolInput {
    /// Path.
    pub path: String,
    /// Edits.
    pub edits: Vec<ReplaceEdit>,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// One authored text replacement.
pub struct ReplaceEdit {
    /// Old text.
    pub old_text: String,
    /// New text.
    pub new_text: String,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Observations supplied by the edit tool.
pub struct EditToolDetails {
    /// Diff.
    pub diff: String,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    /// First changed line.
    pub first_changed_line: Presence<f64>,
}
