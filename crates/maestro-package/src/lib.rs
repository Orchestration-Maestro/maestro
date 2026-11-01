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
#[derive(Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPinStatus {
    /// Commit named by the pin, absent when unavailable locally.
    pub expected_commit: Option<String>,
    /// Current checkout commit, absent when the checkout is missing.
    pub head_commit: Option<String>,
    /// Sorted relative paths reset or removed by restoration.
    pub changed_paths: Vec<String>,
}

impl GitPinStatus {
    /// Whether the checkout exists at its pin with no tracked or untracked drift.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.expected_commit.is_some()
            && self.expected_commit == self.head_commit
            && self.changed_paths.is_empty()
    }
}

impl Serialize for GitPinStatus {
    /// Includes the owner's computed cleanliness alongside the observed fields.
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("GitPinStatus", 4)?;
        state.serialize_field("expectedCommit", &self.expected_commit)?;
        state.serialize_field("headCommit", &self.head_commit)?;
        state.serialize_field("changedPaths", &self.changed_paths)?;
        state.serialize_field("clean", &self.is_clean())?;
        state.end()
    }
}
