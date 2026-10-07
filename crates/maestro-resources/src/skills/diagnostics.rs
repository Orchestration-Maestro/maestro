//! Diagnostics from resource discovery.

/// First-wins collision between resource paths.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceCollision {
    /// Extensible resource kind.
    pub resource_type: String,
    /// Declared name or directory basename.
    pub name: String,
    /// Path of the retained resource.
    pub winner_path: String,
    /// Path of the omitted resource.
    pub loser_path: String,
    /// Optional provenance of the winner.
    pub winner_source: Option<String>,
    /// Optional provenance of the loser.
    pub loser_source: Option<String>,
}

/// Warning or collision observed while loading resources.
#[derive(Debug, Clone, PartialEq)]
pub struct ResourceDiagnostic {
    /// Extensible diagnostic kind.
    pub r#type: String,
    /// Human-readable diagnostic text.
    pub message: String,
    /// Resource path.
    pub path: Option<String>,
    /// Optional collision details.
    pub collision: Option<ResourceCollision>,
}
