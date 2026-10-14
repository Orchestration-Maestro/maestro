//! Content-generation transport preparation and one-attempt response admission.

use indexmap::IndexMap;
use serde_json::Value;

use super::ProjectedContentRequest;
use crate::{DiagnosticErrorInfo, HttpRequest};

/// Captured endpoint, version, key and exact-spelling headers; no transport is installed.
pub struct ApiClient {
    /// Selected constructor endpoint.
    pub(in crate::providers::content) base_url: String,
    /// Selected constructor API version.
    pub(in crate::providers::content) api_version: String,
    /// Supplied authentication key.
    pub(in crate::providers::content) api_key: String,
    /// Constructor header layers after exact-spelling replacement.
    pub(in crate::providers::content) headers: IndexMap<String, String>,
}

/// A prepared POST request and its selected positive timeout in milliseconds.
pub struct PreparedContentRequest {
    /// Request ready for a caller-supplied transport and cancellation handle.
    pub request: HttpRequest,
    /// Positive timeout milliseconds; no timer has been started.
    pub timeout: Option<f64>,
}

impl ApiClient {
    /// Prepare a projected body without sending or installing cancellation listeners.
    ///
    /// # Errors
    /// Fails when the selected URL, headers or authentication cannot be prepared.
    pub fn prepare_request(
        &self,
        mut projected: ProjectedContentRequest,
        http_options: Option<&Value>,
    ) -> Result<PreparedContentRequest, DiagnosticErrorInfo> {
        let url = self.request_url(&projected.model, http_options)?;
        let timeout = http_options
            .and_then(|options| options.get("timeout"))
            .and_then(Value::as_f64)
            .filter(|value| value.is_finite() && *value > 0.0);
        if let Some(Value::Object(extra)) =
            http_options.and_then(|options| options.get("extraBody"))
        {
            merge_body(&mut projected.body, extra);
        }
        let headers = self.request_headers(http_options, timeout)?;
        Ok(PreparedContentRequest {
            request: HttpRequest {
                method: "POST".to_owned(),
                url,
                headers,
                body: maestro_json::compact_object(&projected.body)
                    .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?
                    .into_bytes(),
                signal: None,
            },
            timeout,
        })
    }

    /// Concatenate the selected endpoint, version and resource before URL parsing.
    fn request_url(
        &self,
        model: &str,
        options: Option<&Value>,
    ) -> Result<String, DiagnosticErrorInfo> {
        let endpoint = selected_text(options, "baseUrl", &self.base_url)?;
        let selected_version = selected_text(options, "apiVersion", &self.api_version)?;
        let base = endpoint.strip_suffix('/').unwrap_or(endpoint);
        let version = if selected_version.is_empty() {
            String::new()
        } else {
            format!("{selected_version}/")
        };
        let url = format!("{base}/{version}{model}:streamGenerateContent?alt=sse");
        let mut url = url::Url::parse(&url)
            .map_err(|error| DiagnosticErrorInfo::error(format!("URL: {error}")))?;
        stream_query(&mut url);
        Ok(url.into())
    }

    /// Validate surviving authored fields before timeout and authentication insertion.
    fn request_headers(
        &self,
        http_options: Option<&Value>,
        timeout: Option<f64>,
    ) -> Result<IndexMap<String, String>, DiagnosticErrorInfo> {
        let mut authored =
            IndexMap::from([("Content-Type".to_owned(), "application/json".to_owned())]);
        authored.extend(
            self.headers
                .iter()
                .map(|(name, value)| (name.clone(), value.clone())),
        );
        if let Some(Value::Object(entries)) =
            http_options.and_then(|options| options.get("headers"))
        {
            for (name, value) in entries {
                let value = value
                    .as_str()
                    .ok_or_else(|| DiagnosticErrorInfo::error("Header: value must be text"))?;
                authored.insert(name.clone(), value.to_owned());
            }
        }
        let mut headers = IndexMap::new();
        crate::providers::http::layer_request_headers(&mut headers, &authored, true)
            .map_err(|error| DiagnosticErrorInfo::error(format!("Header: {error}")))?;
        if let Some(milliseconds) = timeout {
            let seconds = maestro_json::compact_json(&Value::from((milliseconds / 1000.0).ceil()))
                .map_err(|error| DiagnosticErrorInfo::error(error.to_string()))?;
            crate::providers::http::layer_request_headers(
                &mut headers,
                &IndexMap::from([("X-Server-Timeout".to_owned(), seconds)]),
                true,
            )
            .map_err(|error| DiagnosticErrorInfo::error(format!("Header: {error}")))?;
        }
        if self.api_key.starts_with("auth_tokens/") {
            return Err(DiagnosticErrorInfo::error(
                "Authentication: ephemeral tokens are not supported",
            ));
        }
        if !headers.contains_key("x-goog-api-key") {
            crate::providers::http::layer_request_headers(
                &mut headers,
                &IndexMap::from([("x-goog-api-key".to_owned(), self.api_key.clone())]),
                true,
            )
            .map_err(|error| DiagnosticErrorInfo::error(format!("Authentication: {error}")))?;
        }
        headers.sort_keys();
        Ok(headers)
    }
}

/// Select a supplied text field, retaining the constructor value only when absent.
fn selected_text<'a>(
    options: Option<&'a Value>,
    key: &str,
    default: &'a str,
) -> Result<&'a str, DiagnosticErrorInfo> {
    match options.and_then(|options| options.get(key)) {
        None => Ok(default),
        Some(Value::String(text)) => Ok(text),
        Some(_) => Err(DiagnosticErrorInfo::error(format!(
            "URL: {key} must be text"
        ))),
    }
}

/// Merge only object/object pairs, replacing other values without moving existing keys.
fn merge_body(target: &mut serde_json::Map<String, Value>, patch: &serde_json::Map<String, Value>) {
    for (key, value) in patch {
        if let Some(Value::Object(current)) = target.get_mut(key)
            && let Value::Object(changes) = value
        {
            merge_body(current, changes);
        } else {
            target.insert(key.clone(), value.clone());
        }
    }
}

/// Replace the first alt entry and discard later alt entries only when its value is not sse.
fn stream_query(url: &mut url::Url) {
    if url
        .query_pairs()
        .find(|(key, _)| key == "alt")
        .is_some_and(|(_, value)| value == "sse")
    {
        return;
    }
    let pairs: Vec<_> = url.query_pairs().into_owned().collect();
    let mut query = url.query_pairs_mut();
    query.clear();
    let mut replaced = false;
    for (key, value) in pairs {
        if key != "alt" {
            query.append_pair(&key, &value);
        } else if !replaced {
            query.append_pair("alt", "sse");
            replaced = true;
        }
    }
    if !replaced {
        query.append_pair("alt", "sse");
    }
}

impl ApiClient {
    /// Send once and consume a failing status before exposing successful response items.
    pub(in crate::providers::content) async fn dispatch(
        &self,
        request: HttpRequest,
        fetch: crate::Fetch,
    ) -> Result<
        impl futures_core::Stream<Item = Result<String, DiagnosticErrorInfo>> + Unpin,
        crate::providers::http::RequestFailure,
    > {
        use crate::providers::http::{RequestFailure, SseMessages, decode_utf8};
        use futures_util::StreamExt;
        let mut response = fetch(request)
            .await
            .map_err(|error| RequestFailure::new(error.to_string()))?;
        if !(200..300).contains(&response.status) {
            let mut bytes = Vec::new();
            while let Some(chunk) = response.body.next().await {
                bytes.extend(chunk.map_err(|error| RequestFailure::new(error.to_string()))?);
            }
            let text = decode_utf8(&bytes);
            let value = if response
                .headers
                .get("content-type")
                .is_some_and(|text| text.contains("application/json"))
            {
                crate::providers::json_text::raw_json(&text)
                    .and_then(crate::providers::json_text::json_value)
                    .map_err(|error| RequestFailure::new(error.to_string()))?
            } else {
                serde_json::json!({"error": {"message": text, "code": response.status, "status": response.status_text}})
            };
            return Err(RequestFailure::new(
                maestro_json::compact_json(&value)
                    .map_err(|error| RequestFailure::new(error.to_string()))?,
            ));
        }
        let source = ContentSource {
            missing: matches!(response.status, 204 | 205),
            body: response.body,
            decoder: SseMessages::data_segments(),
            pending: std::collections::VecDeque::new(),
            ended: false,
        };
        Ok(Box::pin(futures_util::stream::unfold(
            source,
            ContentSource::next,
        )))
    }
}

/// Ordered response-item source retaining pending frames before a later failure.
struct ContentSource {
    /// Transport body.
    body: crate::HttpBody,
    /// Shared byte-framing owner.
    decoder: crate::providers::http::SseMessages,
    /// Completed frames waiting for reduction.
    pending: std::collections::VecDeque<Result<String, DiagnosticErrorInfo>>,
    /// Whether the transport source has ended.
    ended: bool,
    /// Whether the successful status has no response body.
    missing: bool,
}

impl ContentSource {
    /// Deliver pending items before reading or reporting the next source result.
    async fn next(mut self) -> Option<(Result<String, DiagnosticErrorInfo>, Self)> {
        use futures_util::StreamExt;
        if self.missing {
            self.ended = true;
            self.missing = false;
            self.pending
                .push_back(Err(DiagnosticErrorInfo::error("Response body is empty")));
        }
        loop {
            if let Some(item) = self.pending.pop_front() {
                return Some((item, self));
            }
            if self.ended {
                return None;
            }
            match self.body.next().await {
                Some(Ok(bytes)) => self.pending.extend(
                    self.decoder
                        .push(&bytes)
                        .into_iter()
                        .map(|event| Ok(response_data(event.data))),
                ),
                Some(Err(error)) => {
                    self.ended = true;
                    self.pending
                        .push_back(Err(DiagnosticErrorInfo::error(error.to_string())));
                }
                None => {
                    self.ended = true;
                    let tail = self.decoder.segment_tail();
                    self.pending.extend(
                        (!tail
                            .trim_matches(maestro_text::is_whitespace_scalar)
                            .is_empty())
                        .then(|| Err(tail_failure(&tail))),
                    );
                }
            }
        }
    }
}

/// Normalize a null root to the empty response selected by the transport adapter.
fn response_data(data: String) -> String {
    if crate::providers::json_text::raw_json(&data).is_ok_and(|raw| raw.get() == "null") {
        "{}".to_owned()
    } else {
        data
    }
}

/// Classify a residual bare provider error after the entire source is consumed.
fn tail_failure(text: &str) -> DiagnosticErrorInfo {
    let value = serde_json::from_str::<Value>(text).ok();
    let code = value
        .as_ref()
        .and_then(|value| value.get("error"))
        .and_then(|error| error.get("code"))
        .and_then(|code| {
            code.as_f64().or_else(|| {
                code.as_str()?
                    .trim_matches(maestro_text::is_whitespace_scalar)
                    .parse()
                    .ok()
            })
        });
    if code.is_some_and(|code| (400.0..600.0).contains(&code))
        && let Some(value) = value
        && let Ok(body) = maestro_json::compact_json(&value)
    {
        let status = value["error"]["status"].as_str().unwrap_or("undefined");
        return DiagnosticErrorInfo::error(format!("got status: {status}. {body}"));
    }
    DiagnosticErrorInfo::error("Incomplete JSON segment at the end")
}
