//! Observations of managed pinned Git contents.
/// Exact commit selection and paths that differ from the requested pin.
#[derive(Debug, Default, PartialEq, Eq)]
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
