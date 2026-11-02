//! Invocation settings shared by native and authored providers.
use super::{JsonObject, ThinkingLevel};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
/// Capability representations retained by common invocation options.
pub trait StreamOptionSlots {
    /// Cooperative cancellation handle.
    type Signal: Clone;
    /// Awaited payload selector.
    type PayloadHook: Clone;
    /// Awaited response observer.
    type ResponseHook: Clone;
    /// Replacement HTTP transport.
    type Fetch: Clone;
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
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
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
///
/// A concrete fetch slot lets native aliases coerce closures before family inference.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", bound = "")]
pub struct StreamOptions<E: StreamOptionSlots, F = <E as StreamOptionSlots>::Fetch> {
    /// Temperature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Max tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<f64>,
    /// Signal.
    #[serde(skip)]
    pub signal: Option<E::Signal>,
    /// Api key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Request endpoint override applied to the dispatched descriptor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<Transport>,
    /// Cache retention.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_retention: Option<CacheRetention>,
    /// Session id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// On payload.
    #[serde(skip)]
    pub on_payload: Option<E::PayloadHook>,
    /// On response.
    #[serde(skip)]
    pub on_response: Option<E::ResponseHook>,
    /// Headers, applied in insertion order; values are interpreted by the transport.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
    /// Timeout ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<f64>,
    /// Max retries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retries: Option<f64>,
    /// Max retry delay ms.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_retry_delay_ms: Option<f64>,
    /// Metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JsonObject>,
    /// Replacement for the default HTTP transport.
    #[serde(skip)]
    pub fetch: Option<F>,
}
impl<E: StreamOptionSlots, F> Default for StreamOptions<E, F> {
    fn default() -> Self {
        Self {
            temperature: None,
            max_tokens: None,
            signal: None,
            api_key: None,
            base_url: None,
            transport: None,
            cache_retention: None,
            session_id: None,
            on_payload: None,
            on_response: None,
            headers: None,
            timeout_ms: None,
            max_retries: None,
            max_retry_delay_ms: None,
            metadata: None,
            fetch: None,
        }
    }
}
/// Retain typed common options plus provider-specific open fields.
#[derive(Clone, Default)]
pub struct ProviderStreamOptions<C, O> {
    /// Common.
    pub common: C,
    /// Extra.
    pub extra: JsonObject,
    /// Typed objects that cannot travel as JSON, such as an injected client.
    pub objects: O,
}
/// Carry common options plus requested thinking and optional budgets.
#[derive(Clone)]
pub struct SimpleStreamOptions<C, T> {
    /// Common.
    pub common: C,
    /// Reasoning.
    pub reasoning: Option<ThinkingLevel>,
    /// Thinking budgets.
    pub thinking_budgets: Option<ThinkingBudgets>,
    /// Tool selection forwarded by adapters that support it.
    pub tool_choice: Option<T>,
}

impl<C: Default, T> Default for SimpleStreamOptions<C, T> {
    fn default() -> Self {
        Self {
            common: C::default(),
            reasoning: None,
            thinking_budgets: None,
            tool_choice: None,
        }
    }
}

/// Original callback options without conversion between raw and simple policies.
#[derive(Clone)]
pub enum StreamOptionsInput<C, O, T> {
    /// Common controls, open provider fields and retained opaque objects.
    Raw(ProviderStreamOptions<C, O>),
    /// Common controls with reasoning, budgets and tool selection.
    Simple(SimpleStreamOptions<C, T>),
}

impl<C, O, T> StreamOptionsInput<C, O, T> {
    /// Borrow the common controls without changing the supplied variant.
    #[must_use]
    pub fn common(&self) -> &C {
        match self {
            Self::Raw(options) => &options.common,
            Self::Simple(options) => &options.common,
        }
    }
}

impl<C, O, T> From<ProviderStreamOptions<C, O>> for StreamOptionsInput<C, O, T> {
    fn from(options: ProviderStreamOptions<C, O>) -> Self {
        Self::Raw(options)
    }
}

impl<C, O, T> From<SimpleStreamOptions<C, T>> for StreamOptionsInput<C, O, T> {
    fn from(options: SimpleStreamOptions<C, T>) -> Self {
        Self::Simple(options)
    }
}
