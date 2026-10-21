//! HTTPS sources share HTTP cache semantics and the native sender's TLS configuration.

use super::{Cache, Failure, Source};

/// Delegate secure source loading without a second cache or transport implementation.
pub(super) async fn load(
    uri: &url::Url,
    cache: Option<&Cache>,
    fetch: &super::Fetch,
) -> Result<Source, Failure> {
    super::http::load(uri, cache, fetch).await
}
