//! Supplied write tool data.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// Arguments supplied to the write tool.
pub struct WriteToolInput {
    /// Path.
    pub path: String,
    /// Content.
    pub content: String,
    #[serde(flatten)]
    /// Extra.
    pub extra: serde_json::Map<String, serde_json::Value>,
}
