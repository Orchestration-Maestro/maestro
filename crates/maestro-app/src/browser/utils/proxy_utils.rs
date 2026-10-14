//! Provider-specific CORS proxy selection.

/// Whether a provider and credential require the configured proxy.
#[must_use]
pub fn should_use_proxy_for_provider(provider: &str, api_key: &str) -> bool {
    match provider.to_lowercase().as_str() {
        "zai" | "openai-codex" => true,
        "anthropic" => api_key.starts_with("sk-ant-oat") || api_key.starts_with('{'),
        _ => false,
    }
}

use maestro_models::Model;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// URI component punctuation left literal in a proxy target.
const COMPONENT_ENCODING: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// Rewrite only the model base URL when the supplied proxy is required.
#[must_use]
pub fn apply_proxy_if_needed(mut model: Model, api_key: &str, proxy_url: Option<&str>) -> Model {
    if let Some(proxy) = proxy_url.filter(|url| !url.is_empty())
        && !model.base_url.is_empty()
        && should_use_proxy_for_provider(&model.provider, api_key)
    {
        model.base_url = format!("{proxy}/?url={}", encode_uri_component(&model.base_url));
    }
    model
}

/// Whether an error suggests a browser CORS failure rather than ordinary connectivity.
#[must_use]
pub fn is_cors_error(error: &(dyn std::error::Error + 'static)) -> bool {
    let diagnostic = if let Some(transport) = error.downcast_ref::<maestro_models::FetchError>() {
        match transport {
            maestro_models::FetchError::Connection(cause) => Some(cause),
            maestro_models::FetchError::Timeout | maestro_models::FetchError::Aborted => {
                return false;
            }
        }
    } else {
        error.downcast_ref::<maestro_models::DiagnosticErrorInfo>()
    };
    let message = diagnostic.map_or_else(
        || error.to_string().to_lowercase(),
        |cause| cause.message.to_lowercase(),
    );
    diagnostic.is_some_and(|cause| {
        cause.name.as_deref() == Some("NetworkError")
            || (cause.name.as_deref() == Some("TypeError") && message.contains("failed to fetch"))
    }) || message.contains("cors")
        || message.contains("cross-origin")
}

use maestro_agent::StreamFn;
use maestro_models::{BoxFuture, DiagnosticErrorInfo};
use std::sync::Arc;

/// Read current proxy settings for one invocation.
#[cfg(not(target_arch = "wasm32"))]
pub type GetProxyUrl =
    Arc<dyn Fn() -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>> + Send + Sync>;
/// Read browser-local proxy settings for one invocation.
#[cfg(target_arch = "wasm32")]
pub type GetProxyUrl = Arc<dyn Fn() -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>>>;

/// Create a simple-stream adapter that reads proxy settings on each invocation.
#[must_use]
pub fn create_stream_fn(get_proxy_url: GetProxyUrl) -> StreamFn {
    Arc::new(move |model, context, options, _| {
        let get_proxy_url = Arc::clone(&get_proxy_url);
        Box::pin(async move {
            let proxy_url = get_proxy_url().await?;
            let model = match options
                .common
                .api_key
                .as_deref()
                .filter(|key| !key.is_empty())
            {
                Some(key) => apply_proxy_if_needed(model, key, proxy_url.as_deref()),
                None => model,
            };
            maestro_models::stream_simple(model, context, Some(options))
        })
    })
}

/// Encode the authored UTF-8 target as a URI component.
pub(crate) fn encode_uri_component(text: &str) -> String {
    utf8_percent_encode(text, COMPONENT_ENCODING).to_string()
}
