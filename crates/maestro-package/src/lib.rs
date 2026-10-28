//! Package interchange records shared by native hosts and guest authors.
use serde::{Deserialize, Serialize};

/// One selected package, even when none of its resources are enabled.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPackage<S> {
    /// Original source spelling, including its pin.
    pub source: String,
    /// Kind-prefixed identity independent of the pin.
    pub identity: String,
    /// Scope used to resolve the contents.
    pub scope: S,
    /// Existing package contents.
    pub installed_path: String,
}

/// Exact commit selection and paths that differ from the requested pin.
#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPinStatus {
    /// Commit named by the pin, absent when unavailable locally.
    pub expected_commit: Option<String>,
    /// Current checkout commit, absent when the checkout is missing.
    pub head_commit: Option<String>,
    /// Sorted relative paths reset or removed by restoration.
    pub changed_paths: Vec<String>,
}
