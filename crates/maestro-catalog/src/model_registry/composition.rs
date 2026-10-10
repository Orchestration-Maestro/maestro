//! Typed model selection and ordered descriptor composition.
use maestro_models::{Model, ModelCompat, ModelCost, ModelInput, ThinkingLevelMap};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Provider fields consumed after whole-document checking.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProviderConfig {
    /// Endpoint override and custom-model default.
    pub base_url: Option<String>,
    /// Protocol default.
    pub api: Option<String>,
    /// Presence of a configured key, used only by additional validation.
    pub api_key: Option<String>,
    /// Header presence, without retaining unconsumed values.
    pub headers: Option<serde::de::IgnoredAny>,
    /// Open compatibility defaults.
    pub compat: Option<ModelCompat>,
    /// Authored custom definitions.
    #[serde(default)]
    pub models: Vec<ModelDefinition>,
    /// Built-in metadata overrides keyed by exact model ID.
    #[serde(default)]
    pub model_overrides: HashMap<String, ModelOverride>,
}
/// A custom model's identity, defaults and metadata.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModelDefinition {
    /// Exact model identifier.
    pub id: String,
    /// Model-specific protocol.
    pub api: Option<String>,
    /// Model-specific endpoint.
    base_url: Option<String>,
    /// Selected metadata, also used by built-in overrides.
    #[serde(flatten)]
    pub fields: ModelOverride,
}
/// Optional metadata actually consumed by model composition.
#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ModelOverride {
    /// Display name.
    name: Option<String>,
    /// Reasoning capability.
    reasoning: Option<bool>,
    /// Declared reasoning levels, retaining explicit null.
    #[serde(default, deserialize_with = "thinking")]
    thinking_level_map: Option<ThinkingLevelMap>,
    /// Accepted input kinds in authored order.
    input: Option<Vec<ModelInput>>,
    /// Selected partial prices.
    cost: Option<CostOverride>,
    /// Context size.
    pub context_window: Option<f64>,
    /// Output limit.
    pub max_tokens: Option<f64>,
    /// Open compatibility metadata.
    compat: Option<ModelCompat>,
}
/// Individually supplied token prices.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CostOverride {
    /// Input price.
    input: Option<f64>,
    /// Output price.
    output: Option<f64>,
    /// Cache read price.
    cache_read: Option<f64>,
    /// Cache write price.
    cache_write: Option<f64>,
}
/// Select declared thinking keys before decoding their typed values.
fn thinking<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<ThinkingLevelMap>, D::Error> {
    let mut object = Map::<String, Value>::deserialize(decoder)?;
    object.retain(|key, _| {
        matches!(
            key.as_str(),
            "off" | "minimal" | "low" | "medium" | "high" | "xhigh"
        )
    });
    ThinkingLevelMap::deserialize(Value::Object(object))
        .map(Some)
        .map_err(serde::de::Error::custom)
}
/// Enumerate provider keys with canonical numeric indices before other keys.
pub(super) fn providers(value: &Value) -> Result<Vec<(String, ProviderConfig)>, serde_json::Error> {
    let mut entries: Vec<_> = value
        .as_object()
        .and_then(|v| v.get("providers"))
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(Map::iter)
        .collect();
    entries.sort_by_key(|(key, _)| {
        key.parse::<u32>()
            .ok()
            .filter(|n| *n < u32::MAX && n.to_string() == **key)
            .map_or((1, 0), |n| (0, n))
    });
    entries
        .into_iter()
        .map(|(key, value)| ProviderConfig::deserialize(value).map(|config| (key.clone(), config)))
        .collect()
}
/// Apply custom defaults and replace exact identities without moving their first slot.
pub(super) fn compose(
    mut models: Vec<Model>,
    mut providers: Vec<(String, ProviderConfig)>,
) -> Vec<Model> {
    let mut defaults = HashMap::new();
    for model in &models {
        defaults
            .entry(model.provider.clone())
            .or_insert_with(|| (model.api.clone(), model.base_url.clone()));
    }
    let configs: HashMap<_, _> = providers
        .iter()
        .enumerate()
        .map(|(i, (p, _))| (p.clone(), i))
        .collect();
    for model in &mut models {
        if let Some(&index) = configs.get(&model.provider) {
            let config = &mut providers[index].1;
            if let Some(url) = &config.base_url {
                model.base_url.clone_from(url);
            }
            model.compat = merge_compat(model.compat.take(), config.compat.clone());
            if let Some(fields) = config.model_overrides.remove(&model.id) {
                apply(model, fields);
            }
        }
    }
    let mut positions: HashMap<_, _> = models
        .iter()
        .enumerate()
        .map(|(i, m)| ((m.provider.clone(), m.id.clone()), i))
        .collect();
    for (provider, mut config) in providers {
        for definition in std::mem::take(&mut config.models) {
            let Some(model) = custom(&provider, &config, definition, defaults.get(&provider))
            else {
                continue;
            };
            let key = (model.provider.clone(), model.id.clone());
            if let Some(&position) = positions.get(&key) {
                models[position] = model;
            } else {
                positions.insert(key, models.len());
                models.push(model);
            }
        }
    }
    models
}
/// Build a custom descriptor from model, provider and first-built-in defaults.
fn custom(
    provider: &str,
    config: &ProviderConfig,
    definition: ModelDefinition,
    inherited: Option<&(String, String)>,
) -> Option<Model> {
    let api = definition
        .api
        .or_else(|| config.api.clone())
        .or_else(|| inherited.map(|d| d.0.clone()))?;
    let base_url = definition
        .base_url
        .or_else(|| config.base_url.clone())
        .or_else(|| inherited.map(|d| d.1.clone()))?;
    let mut model = Model {
        name: definition.id.clone(),
        id: definition.id,
        api,
        provider: provider.to_owned(),
        base_url,
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: config.compat.clone(),
    };
    apply(&mut model, definition.fields);
    Some(model)
}
/// Overlay selected metadata without applying undeclared API or endpoint fields.
fn apply(model: &mut Model, fields: ModelOverride) {
    if let Some(name) = fields.name {
        model.name = name;
    }
    if let Some(reasoning) = fields.reasoning {
        model.reasoning = reasoning;
    }
    if let Some(thinking) = fields.thinking_level_map {
        model
            .thinking_level_map
            .get_or_insert_default()
            .extend(thinking);
    }
    if let Some(input) = fields.input {
        model.input = input;
    }
    if let Some(context) = fields.context_window {
        model.context_window = context;
    }
    if let Some(tokens) = fields.max_tokens {
        model.max_tokens = tokens;
    }
    if let Some(cost) = fields.cost {
        model.cost.input = cost.input.unwrap_or(model.cost.input);
        model.cost.output = cost.output.unwrap_or(model.cost.output);
        model.cost.cache_read = cost.cache_read.unwrap_or(model.cost.cache_read);
        model.cost.cache_write = cost.cache_write.unwrap_or(model.cost.cache_write);
    }
    model.compat = merge_compat(model.compat.take(), fields.compat);
}
/// Overlay compatibility with one-level routing-object merges.
fn merge_compat(base: Option<ModelCompat>, overlay: Option<ModelCompat>) -> Option<ModelCompat> {
    let Some(overlay) = overlay else {
        return base;
    };
    let mut base = base.unwrap_or_default();
    for (key, value) in overlay.0 {
        if matches!(key.as_str(), "openRouterRouting" | "vercelGatewayRouting") {
            let slot = base.0.entry(key).or_insert(Value::Null);
            let mut routing = match std::mem::take(slot) {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            if let Value::Object(value) = value {
                routing.extend(value);
            }
            *slot = Value::Object(routing);
        } else {
            base.0.insert(key, value);
        }
    }
    Some(base)
}
