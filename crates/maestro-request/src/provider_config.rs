//! Provider registration records shared by the native catalog and extensions.
use crate::types::{Api, ModelCompat, ModelCost, ModelInput, ThinkingLevelMap};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Provider registration; each absent field keeps its earlier value on replacement.
///
/// `C` holds the platform's callable operations, which never enter the JSON form.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig<C = ()> {
    /// Provider display label, including an empty label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Provider endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Configured request key expression.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Protocol used by the stream callback and the model fallback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<Api>,
    /// Request header expressions in supplied order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
    /// Whether to synthesize Authorization.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_header: Option<bool>,
    /// Complete model definitions; an empty list does not replace immediately.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<ProviderModelConfig>>,
    /// Callable operations supplied beside the data.
    #[serde(skip)]
    pub callbacks: C,
}

/// Complete metadata for a dynamically supplied model, without file-loader defaults.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderModelConfig {
    /// Exact model identifier.
    pub id: String,
    /// Display label.
    pub name: String,
    /// Nonempty protocol override.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api: Option<Api>,
    /// Endpoint override, including an empty endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Whether reasoning is supported.
    pub reasoning: bool,
    /// Provider-specific thinking level values.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_level_map: Option<ThinkingLevelMap>,
    /// Accepted input kinds in supplied order.
    pub input: Vec<ModelInput>,
    /// Supplied usage prices.
    pub cost: ModelCost,
    /// Context capacity.
    pub context_window: f64,
    /// Output capacity.
    pub max_tokens: f64,
    /// Per-model request header expressions, not descriptor headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<IndexMap<String, String>>,
    /// Open compatibility metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compat: Option<ModelCompat>,
}
