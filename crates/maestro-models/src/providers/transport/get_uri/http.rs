//! HTTP source loading, validators and retained redirect observations.

use std::collections::BTreeMap;
use std::time::SystemTime;

use super::{
    Cache, Failure, Source, http_error::HttpError, notfound::NotFound, notmodified::NotModified,
};
use crate::providers::transport::{Fetch, HttpRequest, collect_body};

/// One response observation, including redirect responses with their own validators.
#[derive(Debug)]
pub(in crate::providers::transport) struct Observation {
    /// Complete source URL used by the response.
    uri: url::Url,
    /// Status decides whether a fresh observation redirects or reuses source bytes.
    status: u16,
    /// Lowercase response fields from the shared HTTP owner.
    headers: BTreeMap<String, String>,
    /// Header arrival instant used for max-age.
    received: SystemTime,
}

/// Load a source, following at most five redirects without recursive proxy selection.
pub(super) async fn load(
    uri: &url::Url,
    cache: Option<&Cache>,
    fetch: &Fetch,
) -> Result<Source, Failure> {
    let mut uri = uri.clone();
    let mut observations = Vec::new();
    loop {
        let previous = cached(&uri, cache);
        if let Some(old) = previous.filter(|old| fresh(old, SystemTime::now())) {
            if let Some(target) = redirect(old).filter(|_| observations.len() < 5) {
                uri = target?;
                // The old redirect remains owned by the committed cache until a new source loads.
                observations.push(Observation {
                    uri: old.uri.clone(),
                    status: old.status,
                    headers: old.headers.clone(),
                    received: old.received,
                });
                continue;
            }
            if redirect(old).is_some() {
                return Err(HttpError(old.status).into());
            }
            return Err(NotModified.into());
        }
        let response = fetch(request(&uri, previous)).await?;
        let observation = Observation {
            uri: uri.clone(),
            status: response.status,
            headers: response.headers,
            received: SystemTime::now(),
        };
        if let Some(target) = redirect(&observation).filter(|_| observations.len() < 5) {
            uri = target?;
            observations.push(observation);
            drop(response.body);
            continue;
        }
        match observation.status {
            200..=299 => {
                observations.push(observation);
                return Ok(Source {
                    bytes: collect_body(response.body).await?,
                    cache: Cache::Http(observations),
                });
            }
            304 => return Err(NotModified.into()),
            404 => return Err(NotFound.into()),
            status => return Err(HttpError(status).into()),
        }
    }
}

/// Find only the response belonging to the selected URL.
fn cached<'a>(uri: &url::Url, cache: Option<&'a Cache>) -> Option<&'a Observation> {
    match cache {
        Some(Cache::Http(observations)) => observations.iter().rev().find(|old| old.uri == *uri),
        _ => None,
    }
}

/// Build a source GET with matching response validators only.
fn request(uri: &url::Url, previous: Option<&Observation>) -> HttpRequest {
    let mut headers = indexmap::IndexMap::new();
    if let Some(previous) = previous {
        for (source, destination) in [
            ("last-modified", "if-modified-since"),
            ("etag", "if-none-match"),
        ] {
            if let Some(value) = previous.headers.get(source) {
                headers.insert(destination.into(), value.clone());
            }
        }
    }
    HttpRequest {
        method: "GET".into(),
        url: uri.to_string(),
        headers,
        body: Vec::new(),
        signal: None,
    }
}

/// A redirect keeps its own URL resolution base and response identity.
fn redirect(observation: &Observation) -> Option<Result<url::Url, url::ParseError>> {
    (observation.status / 100 == 3)
        .then(|| observation.headers.get("location"))
        .flatten()
        .filter(|location| !location.is_empty())
        .map(|location| observation.uri.join(location))
}

/// The source separator uses interpreter whitespace and ASCII word boundaries.
static DIRECTIVES: std::sync::LazyLock<Result<regex::Regex, regex::Error>> =
    std::sync::LazyLock::new(|| {
        regex::Regex::new(&format!(r",[{}]*?(?-u:\b)", maestro_text::WHITESPACE))
    });

/// Signed epoch milliseconds match source clock observation precision.
fn milliseconds(time: SystemTime) -> f64 {
    time.duration_since(std::time::UNIX_EPOCH).map_or_else(
        |error| -error.duration().as_secs_f64().mul_add(1000.0, 0.0).floor(),
        |duration| duration.as_secs_f64().mul_add(1000.0, 0.0).floor(),
    )
}

/// Preserve directive order and case, with HTTP-date Expires corrected at this owner.
fn fresh(observation: &Observation, now: SystemTime) -> bool {
    if let Some(control) = observation
        .headers
        .get("cache-control")
        .filter(|s| !s.is_empty())
    {
        let Ok(separator) = DIRECTIVES.as_ref() else {
            return false;
        };
        for part in separator.split(control) {
            let mut fields = part.split('=');
            match fields.next().unwrap_or_default() {
                "max-age" => {
                    let age = milliseconds(now) - milliseconds(observation.received);
                    return fields
                        .next()
                        .and_then(decimal)
                        .is_some_and(|seconds| age < seconds * 1000.0);
                }
                "no-cache" | "no-store" => return false,
                _ => {}
            }
        }
        false
    } else {
        observation
            .headers
            .get("expires")
            .and_then(|value| httpdate::parse_http_date(value).ok())
            .is_some_and(|expires| now < expires)
    }
}

/// Parse the decimal prefix used by cache max-age, retaining invalid and signed values.
fn decimal(value: &str) -> Option<f64> {
    let value = value.trim_start_matches(maestro_text::is_whitespace_scalar);
    let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
    let length = digits.bytes().take_while(u8::is_ascii_digit).count();
    if length == 0 {
        return None;
    }
    value[..value.len() - digits.len() + length].parse().ok()
}

#[cfg(test)]
mod tests;
