//! Script source loading with protocol-owned cache observations.

mod data;
mod file;
mod ftp;
mod http;
mod http_error;
mod https;
mod listing;
mod notfound;
pub(super) mod notmodified;

use super::Fetch;

/// Native source failures retain their original error values.
pub(super) type Failure = Box<dyn std::error::Error + Send + Sync>;

/// Observation committed only alongside a successfully compiled script.
#[derive(Debug)]
pub(super) enum Cache {
    /// Exact data URL identity, without a collision-prone digest.
    Data(String),
    /// HTTP responses with independent final and redirect cache identities.
    Http(Vec<http::Observation>),
    /// Signed epoch side and millisecond magnitude from the opened file.
    File(Result<u128, u128>),
    /// Modification instant of the last successfully compiled FTP script.
    Ftp(Option<i64>),
}

/// Loaded script bytes and their candidate cache observation.
#[derive(Debug)]
pub(super) struct Source {
    /// Uninterpreted source payload.
    pub(super) bytes: Vec<u8>,
    /// Metadata not yet published by the resolver owner.
    pub(super) cache: Cache,
}

/// Load one source without recursively selecting a script proxy.
pub(super) async fn load(
    uri: &url::Url,
    cache: Option<&Cache>,
    fetch: &Fetch,
) -> Result<Source, Failure> {
    match uri.scheme() {
        "data" => data::load(uri, cache),
        "file" => file::load(uri, cache).await,
        "ftp" => ftp::load(uri, cache).await,
        "http" => http::load(uri, cache, fetch).await,
        "https" => https::load(uri, cache, fetch).await,
        protocol => Err(std::io::Error::other(format!(
            "Unsupported protocol \"{protocol}:\" specified in URI: \"{uri}\""
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests;
