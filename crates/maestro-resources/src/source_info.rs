//! Resource provenance without configuration selection.

use std::path::PathBuf;

/// The scope from which a resource was supplied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceScope {
    /// User configuration material.
    User,
    /// Project configuration material.
    Project,
    /// Explicit temporary material.
    Temporary,
}

/// The origin of the containing resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceOrigin {
    /// A package-provided resource.
    Package,
    /// An independently supplied resource.
    TopLevel,
}

/// Complete provenance accompanying a resolved path.
pub struct PathMetadata {
    /// Extensible source label.
    pub source: String,
    /// Resource scope.
    pub scope: SourceScope,
    /// Resource origin.
    pub origin: SourceOrigin,
    /// Optional containing directory, retaining explicit empty spelling.
    pub base_dir: Option<PathBuf>,
}

/// Provenance associated with a resource file.
#[derive(Debug)]
pub struct SourceInfo {
    /// Supplied resource path.
    pub path: PathBuf,
    /// Extensible source label.
    pub source: String,
    /// Resource scope.
    pub scope: SourceScope,
    /// Resource origin.
    pub origin: SourceOrigin,
    /// Optional containing directory.
    pub base_dir: Option<PathBuf>,
}

/// Provenance inputs with defaults only for synthetic scope and origin.
pub struct SyntheticSourceOptions {
    /// Extensible source label.
    pub source: String,
    /// Explicit scope, or temporary when absent.
    pub scope: Option<SourceScope>,
    /// Explicit origin, or top-level when absent.
    pub origin: Option<SourceOrigin>,
    /// Optional containing directory, retaining explicit empty spelling.
    pub base_dir: Option<PathBuf>,
}

/// Attach synthetic provenance, defaulting scope and origin only.
#[must_use]
pub fn create_synthetic_source_info(path: PathBuf, options: SyntheticSourceOptions) -> SourceInfo {
    create_source_info(
        path,
        PathMetadata {
            source: options.source,
            scope: options.scope.unwrap_or(SourceScope::Temporary),
            origin: options.origin.unwrap_or(SourceOrigin::TopLevel),
            base_dir: options.base_dir,
        },
    )
}

/// Attach explicit provenance to a path without selecting defaults.
#[must_use]
pub fn create_source_info(path: PathBuf, metadata: PathMetadata) -> SourceInfo {
    SourceInfo {
        path,
        source: metadata.source,
        scope: metadata.scope,
        origin: metadata.origin,
        base_dir: metadata.base_dir,
    }
}
