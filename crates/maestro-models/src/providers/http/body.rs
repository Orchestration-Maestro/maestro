//! Complete response collection shared by buffered protocol operations.

use super::{FetchError, HttpBody};
use futures_util::StreamExt;

/// Read every body chunk, leaving classification and cancellation with the caller.
///
/// # Errors
/// Returns the original body-stream failure after any bytes already received.
pub async fn collect_body(mut body: HttpBody) -> Result<Vec<u8>, FetchError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        bytes.extend(chunk?);
    }
    Ok(bytes)
}

/// Materialize a response before returning it to an enclosing attempt deadline.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn buffered(fetch: super::Fetch) -> super::Fetch {
    std::sync::Arc::new(move |request| {
        let fetch = fetch.clone();
        Box::pin(async move {
            let mut response = fetch(request).await?;
            let bytes = collect_body(response.body).await?;
            response.body = Box::pin(futures_util::stream::once(async move { Ok(bytes) }));
            Ok(response)
        })
    })
}
