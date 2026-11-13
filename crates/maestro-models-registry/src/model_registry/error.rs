//! Catalog-owned diagnostics and propagated account failures.
use maestro_models::{DiagnosticErrorInfo, OAuthError};

/// Catalog failure retaining its diagnostic or original account error.
#[derive(Debug)]
pub enum CatalogError {
    /// Authored catalog diagnostic without an account source.
    Diagnostic(Box<DiagnosticErrorInfo>),
    /// Original failure from a delegated account operation.
    OAuth(OAuthError),
}

impl CatalogError {
    /// Borrow the retained diagnostic record.
    #[must_use]
    pub fn diagnostic(&self) -> &DiagnosticErrorInfo {
        match self {
            Self::Diagnostic(diagnostic) => diagnostic,
            Self::OAuth(error) => &error.diagnostic,
        }
    }
}

impl From<DiagnosticErrorInfo> for CatalogError {
    fn from(diagnostic: DiagnosticErrorInfo) -> Self {
        Self::Diagnostic(Box::new(diagnostic))
    }
}

impl std::fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic().message)
    }
}

impl std::error::Error for CatalogError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Diagnostic(_) => None,
            Self::OAuth(error) => Some(error),
        }
    }
}

impl From<OAuthError> for CatalogError {
    fn from(error: OAuthError) -> Self {
        Self::OAuth(error)
    }
}
