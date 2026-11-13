//! HTTP source failures preserve the response status independently of its native label.

/// A non-success response other than unchanged or not found.
#[derive(Debug)]
pub(super) struct HttpError(pub(super) u16);

impl std::fmt::Display for HttpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = reqwest::StatusCode::from_u16(self.0)
            .ok()
            .and_then(|status| status.canonical_reason())
            .unwrap_or_default();
        formatter.write_str(text)
    }
}

impl std::error::Error for HttpError {}
