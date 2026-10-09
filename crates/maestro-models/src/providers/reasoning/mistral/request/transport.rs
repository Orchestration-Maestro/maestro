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

/// One cancellation observer or one retained lifetime deadline.
struct Lifetime {
    /// Pending completion ends setup or the body.
    end: crate::BoxFuture<crate::FetchError>,
}

impl Lifetime {
    /// Create the operation's only timer, unless a caller supplied a signal.
    fn new(signal: Option<&crate::Cancellation>) -> Self {
        let end: crate::BoxFuture<crate::FetchError> = match signal {
            Some(signal) => {
                let cancelled = signal.cancelled();
                Box::pin(async move {
                    cancelled.await;
                    crate::FetchError::Aborted
                })
            }
            None => Box::pin(async {
                crate::providers::http::sleep(std::time::Duration::from_secs(30)).await;
                crate::FetchError::Timeout
            }),
        };
        Self { end }
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
        let result = if let Poll::Ready(error) = lifetime.end.as_mut().poll(context) {
            Poll::Ready(Some(Err(error)))
        } else {
            body.as_mut().poll_next(context)
        };
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
    let mut lifetime = Lifetime::new(prepared.request.signal.as_ref());
    let early =
        std::future::poll_fn(|context| std::task::Poll::Ready(lifetime.end.as_mut().poll(context)))
            .await;
    let mut work = (prepared.fetch)(prepared.request);
    if let std::task::Poll::Ready(error) = early {
        return Err(super::errors::setup_failure(error));
    }
    let response = std::future::poll_fn(|context| {
        if let std::task::Poll::Ready(error) = lifetime.end.as_mut().poll(context) {
            return std::task::Poll::Ready(Err(error));
        }
        work.as_mut().poll(context)
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
