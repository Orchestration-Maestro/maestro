//! Shared tool metadata for native and guest callers.
use crate::source_info::SourceInfo;
/// Tool description and caller-selected schema representation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo<P = crate::parameter_schema::ParameterSchema> {
    /// Literal tool name.
    pub name: String,
    /// Supplied description.
    pub description: String,
    /// Supplied parameter schema.
    pub parameters: P,
    /// Discovery provenance.
    pub source_info: SourceInfo,
}
