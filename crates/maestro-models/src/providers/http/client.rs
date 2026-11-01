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
pub(super) fn fetch<B: Into<Option<Vec<u8>>> + Send + 'static>(
    request: HttpRequest<B>,
) -> BoxFuture<Result<HttpResponse, FetchError>> {
    Box::pin(async move { send(build()?, request).await })
}

/// Reuse request and response conversion with a caller-configured native client.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn supplied_fetch(client: reqwest::Client) -> super::Fetch {
    Arc::new(move |request| {
        let client = client.clone();
        Box::pin(async move { send(client, request).await })
    })
}

/// Send one attempt with `client`.
async fn send<B: Into<Option<Vec<u8>>>>(
    client: impl Borrow<reqwest::Client>,
    request: HttpRequest<B>,
) -> Result<HttpResponse, FetchError> {
    let request = prepare(client.borrow(), request)?;
    Ok(finish(
        client.borrow().execute(request).await.map_err(failed)?,
    ))
}

/// Prepare one request for either native sender, including ordinary client defaults.
pub(super) fn prepare<B: Into<Option<Vec<u8>>>>(
    client: &reqwest::Client,
    request: HttpRequest<B>,
) -> Result<reqwest::Request, FetchError> {
    let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(failed)?;
    let mut headers = request.headers;
    let mut url = url::Url::parse(&request.url).map_err(failed)?;
    url.set_fragment(None);
    let mut builder = client.request(method, url);
    for (name, value) in client_pairs(&mut headers).map_err(connection)? {
        builder = builder.header(name, value);
    }
    if let Some(body) = request.body.into() {
        builder = builder.body(body);
    }
    let mut request = builder.build().map_err(failed)?;
    request
        .headers_mut()
        .entry(reqwest::header::ACCEPT)
        .or_insert(reqwest::header::HeaderValue::from_static("*/*"));
    Ok(request)
}

/// Convert a response once, with streaming body reads outside header deadlines.
pub(super) fn finish(response: reqwest::Response) -> HttpResponse {
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
    HttpResponse {
        status,
        status_text,
        headers,
        body,
    }
}

/// Chooses the proxy for a destination URL, or none.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) type ProxySelector =
    Arc<dyn Fn(&url::Url) -> Result<Option<url::Url>, String> + Send + Sync>;

/// Client choices a provider makes for its own requests.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Clone, Default)]
pub(crate) struct Route {
    /// Proxy chosen per destination, checked before each request is sent so that an unusable
    /// choice fails it; without one the client uses none, whatever the environment.
    pub(crate) proxy: Option<ProxySelector>,
    /// Script globals survive connector timeout changes within this bridge.
    pub(crate) pac: Arc<super::pac_proxy_agent::PacProxyAgent>,
    /// Whether the connection speaks HTTP/1 only.
    pub(crate) http1_only: bool,
    /// Deadline for establishing a connection, when supplied by the provider.
    pub(crate) connect_timeout: Option<std::time::Duration>,
    /// Deadline for receiving response headers, when supplied by the provider.
    pub(crate) read_timeout: Option<std::time::Duration>,
}

/// Build a sender over a private client that follows `route` and returns redirects unchanged,
/// for requests that do not use the process-wide client.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn configured_fetch(route: Route) -> super::Fetch {
    let clients = configured_clients(&route);
    let check = route.proxy.clone();
    let read_timeout = route.read_timeout;
    Arc::new(move |request| {
        let clients = clients.clone();
        let check = check.clone();
        let route = route.clone();
        Box::pin(async move {
            let selected = if let Some(select) = check {
                select(&url::Url::parse(&request.url).map_err(failed)?).map_err(connection)?
            } else {
                None
            };
            let (client, socks) = clients?;
            let response = send_selected(client, socks, selected, request, &route);
            if let Some(timeout) = read_timeout {
                tokio::time::timeout(timeout, response)
                    .await
                    .map_err(|_| FetchError::Timeout)?
            } else {
                response.await
            }
        })
    })
}

/// Choose one adapter after destination policy, with no automatic replay.
#[cfg(not(target_arch = "wasm32"))]
async fn send_selected(
    client: reqwest::Client,
    socks: Option<super::proxy_agent::Sender>,
    selected: Option<url::Url>,
    request: HttpRequest,
    route: &Route,
) -> Result<HttpResponse, FetchError> {
    if let Some(source) = selected
        .as_ref()
        .filter(|proxy| proxy.scheme().starts_with("pac+"))
    {
        return route
            .pac
            .request(source, request, route, &client)
            .await
            .map_err(|error| classify(error.as_ref()));
    }
    if selected.is_some_and(|proxy| {
        matches!(
            proxy.scheme(),
            "socks" | "socks4" | "socks4a" | "socks5" | "socks5h"
        )
    }) {
        let request = prepare(&client, request)?.try_into().map_err(failed)?;
        let response = socks
            .ok_or_else(|| connection("Missing SOCKS sender".into()))?
            .request(request)
            .await
            .map_err(|error| classify(error.as_ref()))?;
        Ok(finish(reqwest::Response::from(response)))
    } else {
        send(client, request).await
    }
}

/// Retain both native adapters with one TLS configuration and route policy.
#[cfg(not(target_arch = "wasm32"))]
fn configured_clients(
    route: &Route,
) -> Result<(reqwest::Client, Option<super::proxy_agent::Sender>), FetchError> {
    let mut builder = reqwest::Client::builder()
        .retry(reqwest::retry::never())
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy();
    if let Some(timeout) = route.connect_timeout {
        builder = builder.connect_timeout(timeout);
    }
    if let Some(select) = route.proxy.clone() {
        builder = builder.proxy(reqwest::Proxy::custom(move |url| {
            select(url)
                .ok()
                .flatten()
                .filter(|proxy| matches!(proxy.scheme(), "http" | "https"))
        }));
    }
    let tls = super::proxy_agent::tls(route.http1_only)
        .map_err(|error| connection(chain(error.as_ref())))?;
    let socks = route
        .proxy
        .clone()
        .map(|select| super::proxy_agent::sender(select, tls.clone(), route.connect_timeout));
    let client = builder
        .tls_backend_preconfigured(tls)
        .build()
        .map_err(failed)?;
    Ok((client, socks))
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
pub(super) fn chain(error: &dyn Error) -> String {
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
    // Reqwest retains browser exceptions as `JsValue(Name: message)` in the chain.
    let name = message
        .split_once("JsValue(")
        .and_then(|(_, exception)| exception.split_once(": "))
        .map_or("FetchError", |(name, _)| name)
        .to_owned();
    FetchError::Connection(DiagnosticErrorInfo::new(Some(name), message))
}

/// Classify a client error as a timeout or a connection failure.
fn failed<E: Error + 'static>(error: E) -> FetchError {
    classify(&error)
}

/// Render a failure at the HTTP boundary after classifying its retained causes.
fn classify(error: &(dyn Error + 'static)) -> FetchError {
    let timed_out = error
        .downcast_ref::<reqwest::Error>()
        .is_some_and(reqwest::Error::is_timeout);
    #[cfg(not(target_arch = "wasm32"))]
    let timed_out = timed_out
        || std::iter::successors(Some(error), |error| (*error).source())
            .any(<dyn Error + 'static>::is::<super::proxy_agent::ConnectTimeout>);
    if timed_out {
        FetchError::Timeout
    } else {
        connection(chain(error))
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
