//! Provider-specific URL and header construction.

use crate::providers::http::RequestFailure;
use indexmap::IndexMap;
use url::Url;

/// Resolve the configured origin without imposing final Request restrictions.
pub(in crate::providers::reasoning::mistral) fn base_url(
    base: &str,
) -> Result<Url, RequestFailure> {
    let template = regex::Regex::new(r"\{([a-zA-Z0-9_][a-zA-Z0-9_-]*)\}")
        .map_err(|error| RequestFailure::new(error.to_string()))?;
    if let Some(placeholder) = template.captures(base).and_then(|captures| captures.get(1)) {
        return Err(RequestFailure::new(format!(
            "Parameter '{}' is required",
            placeholder.as_str()
        )));
    }
    Url::parse(if base.is_empty() {
        "https://api.mistral.ai"
    } else {
        base.trim_start_matches('/')
    })
    .map_err(|error| RequestFailure::new(error.to_string()))
}

/// Append the relative resource after trimming trailing path separators.
pub(in crate::providers::reasoning::mistral) fn endpoint(
    base: &Url,
) -> Result<String, RequestFailure> {
    if !base.username().is_empty() || base.password().is_some() {
        return Err(RequestFailure::new(
            "Request URL cannot contain credentials",
        ));
    }
    let mut url = base.clone();
    let path = format!("{}/", url.path().trim_end_matches('/'));
    url.set_path(&path);
    url.set_query(None);
    url.set_fragment(None);
    url.join("v1/chat/completions")
        .map(Into::into)
        .map_err(|error| RequestFailure::new(error.to_string()))
}

/// Validate defaults before authored case variants replace them.
pub(super) fn headers(
    key: &str,
    authored: IndexMap<String, String>,
) -> Result<IndexMap<String, String>, RequestFailure> {
    let bearer = if key
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("bearer "))
    {
        key.to_owned()
    } else {
        format!("Bearer {key}")
    };
    let mut defaults = IndexMap::from([
        ("accept".into(), "text/event-stream".into()),
        ("authorization".into(), bearer),
        ("content-type".into(), "application/json".into()),
        ("cookie".into(), String::new()),
    ]);
    crate::providers::http::normalize_request(&mut defaults).map_err(RequestFailure::new)?;
    let mut combined: IndexMap<String, String> = IndexMap::new();
    for (name, value) in authored {
        let name = name.to_ascii_lowercase();
        let value = value.trim_matches(crate::providers::http::edge_whitespace);
        if let Some(existing) = combined.get_mut(&name) {
            existing.push_str(if name == "cookie" { "; " } else { ", " });
            existing.push_str(value);
        } else {
            combined.insert(name, value.to_owned());
        }
    }
    defaults.extend(combined);
    crate::providers::http::normalize_request(&mut defaults).map_err(RequestFailure::new)?;
    Ok(defaults)
}

/// Match the exact media type before optional parameters.
pub(in crate::providers::reasoning::mistral) fn media_type(
    value: Option<&str>,
    expected: &str,
) -> bool {
    value.is_some_and(|value| {
        super::trim(value)
            .split(';')
            .next()
            .is_some_and(|kind| super::trim(kind).eq_ignore_ascii_case(expected))
    })
}

/// How long a request without a configured timeout and without a caller signal waits for a
/// response or a chunk.
const DEFAULT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// The configured timeout in milliseconds as a duration; a negative or non-numeric one is
/// ignored and one too long to represent never elapses.
pub(in crate::providers::reasoning::mistral) fn timeout(
    milliseconds: Option<f64>,
) -> Option<std::time::Duration> {
    milliseconds
        .filter(|milliseconds| *milliseconds >= 0.0)
        .map(|milliseconds| {
            std::time::Duration::try_from_secs_f64(milliseconds / 1000.0)
                .unwrap_or(std::time::Duration::MAX)
        })
}

/// A timer that ends the wait with a timeout after `limit`.
fn timer(limit: std::time::Duration) -> crate::BoxFuture<crate::FetchError> {
    Box::pin(async move {
        crate::providers::http::sleep(limit).await;
        crate::FetchError::Timeout
    })
}

/// The caller's cancellation observer and the idle timer of one request.
struct Lifetime {
    /// Pending completion when the caller's signal aborts.
    cancelled: Option<crate::BoxFuture<crate::FetchError>>,
    /// The longest wait and the timer measuring the current one.
    idle: Option<(std::time::Duration, crate::BoxFuture<crate::FetchError>)>,
    /// A wait that ran out while it was being started.
    expired: Option<crate::FetchError>,
}

impl Lifetime {
    /// Observe the caller's signal and, when `limit` is set, time the first wait.
    fn new(signal: Option<&crate::Cancellation>, limit: Option<std::time::Duration>) -> Self {
        Self {
            cancelled: signal.map(|signal| {
                let cancelled = signal.cancelled();
                Box::pin(async move {
                    cancelled.await;
                    crate::FetchError::Aborted
                }) as crate::BoxFuture<crate::FetchError>
            }),
            idle: limit.map(|limit| (limit, timer(limit))),
            expired: None,
        }
    }

    /// Start a new wait: the response or the next chunk may take the whole limit again. A
    /// timer's deadline starts at its first poll, so it is polled here.
    fn restart(&mut self, context: &mut std::task::Context<'_>) {
        if let Some((limit, running)) = self.idle.as_mut() {
            *running = timer(*limit);
            if let std::task::Poll::Ready(error) = running.as_mut().poll(context) {
                self.expired = Some(error);
            }
        }
    }

    /// The failure that ends the request now, if the signal aborted or the wait ran out.
    fn poll_end(
        &mut self,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<crate::FetchError> {
        if let Some(error) = self.expired.take() {
            return std::task::Poll::Ready(error);
        }
        if let Some(cancelled) = self.cancelled.as_mut()
            && let ready @ std::task::Poll::Ready(_) = cancelled.as_mut().poll(context)
        {
            return ready;
        }
        match self.idle.as_mut() {
            Some((_, running)) => running.as_mut().poll(context),
            None => std::task::Poll::Pending,
        }
    }
}

/// A response body that releases its carrier after EOF, failure or drop.
struct Body {
    /// Remaining response and its cancellation carrier.
    active: Option<(crate::HttpBody, Lifetime)>,
}

impl futures_core::Stream for Body {
    type Item = Result<Vec<u8>, crate::FetchError>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        context: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        use std::task::Poll;
        let Some((body, lifetime)) = self.active.as_mut() else {
            return Poll::Ready(None);
        };
        let result = if let Poll::Ready(error) = lifetime.poll_end(context) {
            Poll::Ready(Some(Err(error)))
        } else {
            body.as_mut().poll_next(context)
        };
        if matches!(result, Poll::Ready(Some(Ok(_)))) {
            lifetime.restart(context);
        }
        if matches!(result, Poll::Ready(None | Some(Err(_)))) {
            self.active = None;
        }
        result
    }
}

/// Invoke the selected client once and carry its deadline through admission.
pub(in crate::providers::reasoning::mistral) async fn send(
    prepared: super::Prepared,
) -> Result<crate::HttpBody, RequestFailure> {
    use futures_util::StreamExt;
    let limit = prepared
        .timeout
        .or_else(|| prepared.request.signal.is_none().then_some(DEFAULT_TIMEOUT));
    let mut lifetime = Lifetime::new(prepared.request.signal.as_ref(), limit);
    let early =
        std::future::poll_fn(|context| std::task::Poll::Ready(lifetime.poll_end(context))).await;
    let mut work = (prepared.fetch)(prepared.request);
    if let std::task::Poll::Ready(error) = early {
        return Err(super::errors::setup_failure(error));
    }
    let response = std::future::poll_fn(|context| {
        if let std::task::Poll::Ready(error) = lifetime.poll_end(context) {
            return std::task::Poll::Ready(Err(error));
        }
        match work.as_mut().poll(context) {
            std::task::Poll::Ready(Ok(response)) => {
                lifetime.restart(context);
                std::task::Poll::Ready(Ok(response))
            }
            other => other,
        }
    })
    .await
    .map_err(super::errors::setup_failure)?;
    let status = response.status;
    let content_type = response.headers.get("content-type");
    let mut body: crate::HttpBody = Box::pin(Body {
        active: Some((response.body, lifetime)),
    });
    if status == 200 && media_type(content_type.map(String::as_str), "text/event-stream") {
        return Ok(body);
    }
    let matched = (400..600).contains(&status);
    let mut bytes = Vec::new();
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(chunk) => bytes.extend(chunk),
            Err(error) if matched => return Err(RequestFailure::new(error.to_string())),
            Err(_) => {
                bytes.clear();
                break;
            }
        }
    }
    let text = crate::providers::http::decode_utf8(&bytes);
    if status == 422 && media_type(content_type.map(String::as_str), "application/json") {
        crate::providers::json_text::raw_json(&text)
            .map_err(|error| RequestFailure::new(error.to_string()))?;
    }
    let fallback =
        super::errors::fallback(status, content_type.map(String::as_str), &text, matched);
    Err(RequestFailure::new(super::errors::format_error(
        Some(status),
        Some(&text),
        &fallback,
    )))
}
