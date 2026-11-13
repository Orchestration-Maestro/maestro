//! Resource provenance supplied to a request.
use serde::{Deserialize, Serialize};

/// The scope from which a resource was supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceScope {
    /// User configuration material.
    User,
    /// Project configuration material.
    Project,
    /// Explicit temporary material.
    Temporary,
}

/// The origin of the containing resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SourceOrigin {
    /// A package-provided resource.
    Package,
    /// An independently supplied resource.
    TopLevel,
}

/// Provenance associated with a resource file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    /// Supplied resource path.
    pub path: String,
    /// Extensible source label.
    pub source: String,
    /// Resource scope.
    pub scope: SourceScope,
    /// Resource origin.
    pub origin: SourceOrigin,
    /// Optional containing directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_dir: Option<String>,
}
