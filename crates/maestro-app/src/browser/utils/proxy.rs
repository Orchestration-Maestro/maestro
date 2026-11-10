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

use crate::utils::uri::encode_uri_component;
use maestro_models::Model;

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
use maestro_models::SharedHandle;
use maestro_models::{BoxFuture, DiagnosticErrorInfo};

/// Read current proxy settings for one invocation.
#[cfg(not(target_arch = "wasm32"))]
pub type GetProxyUrl =
    SharedHandle<dyn Fn() -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>> + Send + Sync>;
/// Read browser-local proxy settings for one invocation.
#[cfg(target_arch = "wasm32")]
pub type GetProxyUrl =
    SharedHandle<dyn Fn() -> BoxFuture<Result<Option<String>, DiagnosticErrorInfo>>>;

/// Create a simple-stream adapter that reads proxy settings on each invocation.
#[must_use]
pub fn create_stream_fn(get_proxy_url: GetProxyUrl) -> StreamFn {
    SharedHandle::new(move |model, context, options, _| {
        let get_proxy_url = SharedHandle::clone(&get_proxy_url);
        Box::pin(async move {
            let model = proxied(model, options.common.api_key.as_deref(), &get_proxy_url).await?;
            maestro_models::stream_simple(model, context, Some(options))
        })
    })
}
/// Create the same adapter for a local agent, submitting provider work through `spawn`.
#[must_use]
pub fn create_local_stream_fn(
    get_proxy_url: GetProxyUrl,
    spawn: maestro_agent::AgentSpawner<maestro_models::Local>,
) -> StreamFn<maestro_models::Local> {
    use maestro_agent::AgentExecution as _;
    let submission = maestro_models::Local::provider_submission(spawn);
    std::rc::Rc::new(move |model, context, options, _| {
        let get_proxy_url = SharedHandle::clone(&get_proxy_url);
        let submission = submission.clone();
        Box::pin(async move {
            let model = proxied(model, options.common.api_key.as_deref(), &get_proxy_url).await?;
            maestro_models::Local::provider_stream(model, context, options, submission)
        })
    })
}
/// Read the current proxy setting and route `model` through it when a non-empty key needs it.
async fn proxied(
    model: Model,
    api_key: Option<&str>,
    get_proxy_url: &GetProxyUrl,
) -> Result<Model, DiagnosticErrorInfo> {
    let proxy_url = get_proxy_url().await?;
    Ok(match api_key.filter(|key| !key.is_empty()) {
        Some(key) => apply_proxy_if_needed(model, key, proxy_url.as_deref()),
        None => model,
    })
}
