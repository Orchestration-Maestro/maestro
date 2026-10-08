//! Default HTTP client over reqwest.

use std::collections::BTreeMap;
use std::error::Error;

use futures_util::StreamExt;

use super::{FetchError, HttpBody, HttpRequest, HttpResponse};
use crate::{BoxFuture, DiagnosticErrorInfo};

/// Send one attempt with the shared client.
pub(super) fn fetch(request: HttpRequest) -> BoxFuture<Result<HttpResponse, FetchError>> {
    Box::pin(async move {
        let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(failed)?;
        let mut builder = build()?.request(method, &request.url);
        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }
        let response = builder.body(request.body).send().await.map_err(failed)?;
        let status = response.status().as_u16();
        let headers = collect_headers(response.headers());
        let body: HttpBody = Box::pin(
            response
                .bytes_stream()
                .map(|chunk| chunk.map(|bytes| bytes.to_vec()).map_err(failed)),
        );
        Ok(HttpResponse {
            status,
            headers,
            body,
        })
    })
}

/// The process-wide client; automatic retries are off because the sender owns retry policy.
#[cfg(not(target_arch = "wasm32"))]
fn build() -> Result<&'static reqwest::Client, FetchError> {
    use std::sync::OnceLock;

    static CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .retry(reqwest::retry::never())
                .build()
                .map_err(|error| chain(&error))
        })
        .as_ref()
        .map_err(|message| connection(message.clone()))
}

/// A browser client; the platform fetch owns connection reuse.
#[cfg(target_arch = "wasm32")]
fn build() -> Result<reqwest::Client, FetchError> {
    reqwest::Client::builder().build().map_err(failed)
}

/// Copy headers, joining repeated names the way a header map reports them.
fn collect_headers(headers: &reqwest::header::HeaderMap) -> BTreeMap<String, String> {
    let mut copied: BTreeMap<String, String> = BTreeMap::new();
    for (name, value) in headers {
        let value = String::from_utf8_lossy(value.as_bytes());
        copied
            .entry(name.as_str().to_owned())
            .and_modify(|joined| {
                joined.push_str(", ");
                joined.push_str(&value);
            })
            .or_insert_with(|| value.into_owned());
    }
    copied
}

/// Join an error with its causes into one line.
fn chain(error: &dyn Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(next) = cause {
        text.push_str(": ");
        text.push_str(&next.to_string());
        cause = next.source();
    }
    text
}

/// Build the retryable connection failure for a message.
fn connection(message: String) -> FetchError {
    FetchError::Connection(DiagnosticErrorInfo {
        name: Some("FetchError".to_owned()),
        message,
        stack: None,
        code: None,
    })
}

/// Classify a client error as a timeout or a connection failure.
fn failed<E: Error + 'static>(error: E) -> FetchError {
    let timed_out = (&error as &dyn Error)
        .downcast_ref::<reqwest::Error>()
        .is_some_and(reqwest::Error::is_timeout);
    if timed_out {
        FetchError::Timeout
    } else {
        connection(chain(&error))
    }
}
