//! Invocation settings with retained adapter hooks.
use super::{Cancellation, JsonObject, OnPayload, OnResponse};
use serde::{Deserialize, Serialize};
/// Represent the accepted reasoning levels, with Off only on model selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    /// Minimal.
    Minimal,
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Xhigh.
    Xhigh,
}
/// Represent the accepted reasoning levels, with Off only on model selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum ModelThinkingLevel {
    /// Off.
    Off,
    /// Minimal.
    Minimal,
    /// Low.
    Low,
    /// Medium.
    Medium,
    /// High.
    High,
    /// Xhigh.
    Xhigh,
}
/// Carry optional per-level token budgets.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThinkingBudgets {
    /// Minimal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimal: Option<f64>,
    /// Low.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub low: Option<f64>,
    /// Medium.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub medium: Option<f64>,
    /// High.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high: Option<f64>,
}
/// Select none, short or long cache retention.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CacheRetention {
    /// None.
    None,
    /// Short.
    Short,
    /// Long.
    Long,
}
/// Carry the requested response transport.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Transport {
    /// Sse.
    Sse,
    /// Websocket.
    Websocket,
    /// `WebsocketCached`.
    #[serde(rename = "websocket-cached")]
    WebsocketCached,
    /// Auto.
    Auto,
}
/// Expose response status and copied headers to a callback.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProviderResponse {
    /// Status.
    pub status: f64,
    /// Headers.
    pub headers: std::collections::BTreeMap<String, String>,
}
/// Carry common invocation settings, cooperative cancellation and hooks unchanged.
#[derive(Clone, Default)]
pub struct StreamOptions {
    /// Temperature.
    pub temperature: Option<f64>,
    /// Max tokens.
    pub max_tokens: Option<f64>,
    /// Signal.
    pub signal: Option<Cancellation>,
    /// Api key.
    pub api_key: Option<String>,
    /// Transport.
    pub transport: Option<Transport>,
    /// Cache retention.
    pub cache_retention: Option<CacheRetention>,
    /// Session id.
    pub session_id: Option<String>,
    /// On payload.
    pub on_payload: Option<OnPayload>,
    /// On response.
    pub on_response: Option<OnResponse>,
    /// Headers.
    pub headers: Option<std::collections::BTreeMap<String, String>>,
    /// Timeout ms.
    pub timeout_ms: Option<f64>,
    /// Max retries.
    pub max_retries: Option<f64>,
    /// Max retry delay ms.
    pub max_retry_delay_ms: Option<f64>,
    /// Metadata.
    pub metadata: Option<JsonObject>,
}
/// Retain typed common options plus provider-specific open fields.
#[derive(Clone, Default)]
pub struct ProviderStreamOptions {
    /// Common.
    pub common: StreamOptions,
    /// Extra.
    pub extra: JsonObject,
}
/// Carry common options plus requested thinking and optional budgets.
#[derive(Clone, Default)]
pub struct SimpleStreamOptions {
    /// Common.
    pub common: StreamOptions,
    /// Reasoning.
    pub reasoning: Option<ThinkingLevel>,
    /// Thinking budgets.
    pub thinking_budgets: Option<ThinkingBudgets>,
}
