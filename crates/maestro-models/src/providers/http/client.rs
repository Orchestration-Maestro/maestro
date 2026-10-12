//! Default HTTP client over reqwest.

use std::borrow::Borrow;
use std::error::Error;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;

use futures_util::StreamExt;

use super::headers::{client_pairs, response_record};
use super::{FetchError, HttpBody, HttpRequest, HttpResponse};
use crate::{BoxFuture, DiagnosticErrorInfo};

/// Send one attempt with the shared client.
pub(super) fn fetch(request: HttpRequest) -> BoxFuture<Result<HttpResponse, FetchError>> {
    Box::pin(async move { send(build()?, request).await })
}

/// Send one attempt with `client`.
async fn send(
    client: impl Borrow<reqwest::Client>,
    request: HttpRequest,
) -> Result<HttpResponse, FetchError> {
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(failed)?;
    let mut headers = request.headers;
    let mut builder = client.borrow().request(method, &request.url);
    for (name, value) in client_pairs(&mut headers).map_err(connection)? {
        builder = builder.header(name, value);
    }
    let response = builder.body(request.body).send().await.map_err(failed)?;
    let status = response.status().as_u16();
    #[cfg(not(target_arch = "wasm32"))]
    let status_text = status_text(&response);
    #[cfg(target_arch = "wasm32")]
    let status_text = String::new();
    let headers = response_record(response.headers());
    let body: HttpBody = Box::pin(
        response
            .bytes_stream()
            .map(|chunk| chunk.map(|bytes| bytes.to_vec()).map_err(failed)),
    );
    Ok(HttpResponse {
        status,
        status_text,
        headers,
        body,
    })
}

/// Chooses the proxy for a destination URL, or none.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type ProxySelector =
    Arc<dyn Fn(&url::Url) -> Result<Option<url::Url>, String> + Send + Sync>;

/// Client choices a provider makes for its own requests.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub(crate) struct Route {
    /// Proxy chosen per destination, checked before each request is sent so that an unusable
    /// choice fails it; without one the client uses none, whatever the environment.
    pub(crate) proxy: Option<ProxySelector>,
    /// Whether the connection speaks HTTP/1 only.
    pub(crate) http1_only: bool,
}

/// Build a sender over a private client that follows `route` and returns redirects unchanged,
/// for requests that do not use the process-wide client.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn configured_fetch(route: Route) -> super::Fetch {
    let mut builder = reqwest::Client::builder()
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    let check = route.proxy.clone();
    if let Some(select) = route.proxy {
        builder = builder.proxy(reqwest::Proxy::custom(move |url| {
            select(url).ok().flatten()
        }));
    }
    if route.http1_only {
        builder = builder.http1_only();
    }
    let client = builder.build().map_err(|error| connection(chain(&error)));
    Arc::new(move |request| {
        let client = client.clone();
        let check = check.clone();
        Box::pin(async move {
            if let Some(select) = check {
                let destination = url::Url::parse(&request.url).map_err(failed)?;
                select(&destination).map_err(connection)?;
            }
            send(client?, request).await
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

/// Read the native response status text.
#[cfg(not(target_arch = "wasm32"))]
fn status_text(response: &reqwest::Response) -> String {
    if !matches!(
        response.version(),
        reqwest::Version::HTTP_10 | reqwest::Version::HTTP_11
    ) {
        return String::new();
    }
    response
        .extensions()
        .get::<hyper::ext::ReasonPhrase>()
        .map_or_else(
            || {
                response
                    .status()
                    .canonical_reason()
                    .unwrap_or_default()
                    .to_owned()
            },
            |phrase| String::from_utf8_lossy(phrase.as_bytes()).into_owned(),
        )
}

#[cfg(test)]
mod tests;
