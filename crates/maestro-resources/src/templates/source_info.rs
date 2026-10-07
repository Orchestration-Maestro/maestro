//! Resource provenance supplied by callers.

/// Extensible provenance scope.
pub type SourceScope = String;

/// Extensible provenance origin.
pub type SourceOrigin = String;

/// Caller-supplied provenance for resolved paths.
#[derive(Debug, Clone, PartialEq)]
pub struct PathMetadata {
    /// Caller-supplied source identifier.
    pub source: String,
    /// Caller-supplied resource scope.
    pub scope: SourceScope,
    /// Caller-supplied resource origin.
    pub origin: SourceOrigin,
    /// Base directory for relative resource references.
    pub base_dir: Option<String>,
}

/// Provenance attached to one resource path.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceInfo {
    /// Resource path.
    pub path: String,
    /// Caller-supplied source identifier.
    pub source: String,
    /// Caller-supplied resource scope.
    pub scope: SourceScope,
    /// Caller-supplied resource origin.
    pub origin: SourceOrigin,
    /// Base directory for relative resource references.
    pub base_dir: Option<String>,
}

/// Copies supplied metadata for a resource path.
pub fn create_source_info(path: &str, metadata: &PathMetadata) -> SourceInfo {
    SourceInfo {
        path: path.into(),
        source: metadata.source.clone(),
        scope: metadata.scope.clone(),
        origin: metadata.origin.clone(),
        base_dir: metadata.base_dir.clone(),
    }
}
/// Creates provenance with temporary, top-level defaults when omitted.
pub fn create_synthetic_source_info(
    path: &str,
    source: &str,
    scope: Option<&str>,
    origin: Option<&str>,
    base_dir: Option<&str>,
) -> SourceInfo {
    create_source_info(
        path,
        &PathMetadata {
            source: source.into(),
            scope: scope.unwrap_or("temporary").into(),
            origin: origin.unwrap_or("top-level").into(),
            base_dir: base_dir.map(str::to_owned),
        },
    )
}
