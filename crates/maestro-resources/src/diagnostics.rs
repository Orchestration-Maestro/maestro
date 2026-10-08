//! Ordered resource diagnostics without process output.

/// The category of a resource diagnostic.
#[derive(Debug, PartialEq, Eq)]
pub enum DiagnosticType {
    /// Retained validation or loading warning.
    Warning,
    /// Resource error.
    Error,
    /// Duplicate resource name.
    Collision,
}
/// The two resources involved in a name collision.
#[derive(Debug)]
pub struct ResourceCollision {
    /// Extensible resource kind.
    pub resource_type: String,
    /// Conflicting name.
    pub name: String,
    /// Retained file.
    pub winner_path: String,
    /// Omitted file.
    pub loser_path: String,
    /// Optional retained source label.
    pub winner_source: Option<String>,
    /// Optional omitted source label.
    pub loser_source: Option<String>,
}
/// One returned diagnostic associated with a resource.
#[derive(Debug)]
pub struct ResourceDiagnostic {
    /// Diagnostic category.
    pub r#type: DiagnosticType,
    /// Authored message or native error cause.
    pub message: String,
    /// Associated resource path.
    pub path: Option<String>,
    /// Collision details when applicable.
    pub collision: Option<ResourceCollision>,
}
impl ResourceDiagnostic {
    /// Associate a warning with a file.
    pub(crate) fn warning(path: &str, message: impl Into<String>) -> Self {
        Self {
            r#type: DiagnosticType::Warning,
            message: message.into(),
            path: Some(path.into()),
            collision: None,
        }
    }
}
