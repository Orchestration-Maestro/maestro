//! Missing source category independent of transport error wording.

/// The selected file or HTTP endpoint does not exist.
#[derive(Debug)]
pub(super) struct NotFound;

impl std::fmt::Display for NotFound {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("File does not exist at the specified endpoint")
    }
}

impl std::error::Error for NotFound {}
