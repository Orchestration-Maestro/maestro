//! Local proxy configuration and the serialized request allowlist.
use maestro_models::{Context, Model, SimpleStreamOptions};
use serde::Serialize;

/// Proxy endpoint, local authentication and invocation options.
#[derive(Clone)]
pub struct ProxyStreamOptions {
    /// Simple invocation options; only the proxy allowlist is serialized.
    pub options: SimpleStreamOptions,
    /// Local bearer credential, sent only in the HTTP authorization header.
    pub auth_token: String,
    /// Authored endpoint prefix; `/api/stream` is appended literally.
    pub proxy_url: String,
}

/// Borrowed request envelope in wire-field order.
#[derive(Serialize)]
struct Request<'a, H> {
    /// Selected descriptor.
    model: &'a Model,
    /// Conversation supplied by the caller.
    context: &'a Context,
    /// Serializable invocation settings.
    options: Options<'a, H>,
}

/// Allowed invocation fields, in their wire order.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Options<'a, H> {
    /// Sampling temperature.
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    /// Requested output budget.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<f64>,
    /// Requested reasoning effort.
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning: &'a Option<maestro_models::ThinkingLevel>,
    /// Requested cache policy.
    #[serde(skip_serializing_if = "Option::is_none")]
    cache_retention: &'a Option<maestro_models::CacheRetention>,
    /// Caller session identifier.
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: &'a Option<String>,
    /// Provider headers, distinct from proxy HTTP headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    headers: &'a Option<H>,
    /// Open metadata record.
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: &'a Option<maestro_models::JsonObject>,
    /// Requested provider transport.
    #[serde(skip_serializing_if = "Option::is_none")]
    transport: &'a Option<maestro_models::Transport>,
    /// Per-effort budgets.
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_budgets: &'a Option<maestro_models::ThinkingBudgets>,
    /// Forwarded retry delay ceiling.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_retry_delay_ms: Option<f64>,
}

/// Build the canonical proxy body without local hooks or credentials.
pub(super) fn body(
    model: &Model,
    context: &Context,
    options: &SimpleStreamOptions,
) -> Result<Vec<u8>, serde_json::Error> {
    let common = &options.common;
    let request = Request {
        model,
        context,
        options: Options {
            temperature: common.temperature,
            max_tokens: common.max_tokens,
            reasoning: &options.reasoning,
            cache_retention: &common.cache_retention,
            session_id: &common.session_id,
            headers: &common.headers,
            metadata: &common.metadata,
            transport: &common.transport,
            thinking_budgets: &options.thinking_budgets,
            max_retry_delay_ms: common.max_retry_delay_ms,
        },
    };
    maestro_models::compact_json(&serde_json::to_value(request)?).map(String::into_bytes)
}
