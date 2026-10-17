//! Shared tool metadata for native and guest callers.
use crate::source_info::SourceInfo;
/// Tool description and caller-selected schema representation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo<P = serde_json::Value> {
    /// Literal tool name.
    pub name: String,
    /// Supplied description.
    pub description: String,
    /// Supplied parameter schema.
    pub parameters: P,
    /// Discovery provenance.
    pub source_info: SourceInfo,
}
