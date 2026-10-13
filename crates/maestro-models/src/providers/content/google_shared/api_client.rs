//! Pure transport preparation for content-generation requests.

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
