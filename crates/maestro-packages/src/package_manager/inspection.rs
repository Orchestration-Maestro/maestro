//! Read-only release discovery and isolated candidate contents.

/// One Git tag with its peeled commit identity.
#[derive(Debug, PartialEq, Eq)]
pub struct GitRelease {
    /// Tag name without the refs prefix.
    pub name: String,
    /// Commit named by the tag, not an annotated tag object.
    pub commit: String,
}
