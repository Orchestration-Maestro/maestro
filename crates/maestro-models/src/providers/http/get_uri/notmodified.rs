//! Unchanged source observation.

/// The source has not changed since the supplied cache observation.
#[derive(Debug)]
pub(in crate::providers::http) struct NotModified;

impl std::fmt::Display for NotModified {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(
            "Source has not been modified since the provied \"cache\", re-use previous results",
        )
    }
}

impl std::error::Error for NotModified {}
