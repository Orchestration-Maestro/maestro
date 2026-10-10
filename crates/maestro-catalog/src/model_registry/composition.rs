//! Typed model selection and ordered descriptor composition.
use maestro_models::{
    Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel, ThinkingLevelMap,
};
use serde::{
    Deserialize,
    de::{IgnoredAny, MapAccess, Visitor},
};
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
/// A thinking-map key: a declared level, or any other member ignored unread.
#[derive(Deserialize)]
#[serde(untagged)]
enum Level {
    /// Declared reasoning level.
    Declared(ModelThinkingLevel),
    /// Undeclared member.
    Other(IgnoredAny),
}
/// Collects declared thinking levels from a map, skipping other members unread.
struct Selected;
impl<'de> Visitor<'de> for Selected {
    type Value = ThinkingLevelMap;
    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("a thinking level map")
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut levels = ThinkingLevelMap::new();
        while let Some(key) = map.next_key::<Level>()? {
            if let Level::Declared(level) = key {
                levels.insert(level, map.next_value()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(levels)
    }
}
/// Select declared thinking levels without building undeclared values.
fn thinking<'de, D: serde::Deserializer<'de>>(
    decoder: D,
) -> Result<Option<ThinkingLevelMap>, D::Error> {
    decoder.deserialize_map(Selected).map(Some)
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
/// Overlay compatibility; a routing slot merges one level only when either side is truthy.
fn merge_compat(base: Option<ModelCompat>, overlay: Option<ModelCompat>) -> Option<ModelCompat> {
    let Some(overlay) = overlay else {
        return base;
    };
    let mut base = base.unwrap_or_default();
    for (key, value) in overlay.0 {
        let routing = matches!(key.as_str(), "openRouterRouting" | "vercelGatewayRouting");
        if routing && (truthy(base.0.get(&key)) || truthy(Some(&value))) {
            let slot = base.0.entry(key).or_insert(Value::Null);
            let mut merged = match std::mem::take(slot) {
                Value::Object(object) => object,
                _ => Map::new(),
            };
            if let Value::Object(value) = value {
                merged.extend(value);
            }
            *slot = Value::Object(merged);
        } else {
            base.0.insert(key, value);
        }
    }
    Some(base)
}
/// Whether a JSON value is truthy.
fn truthy(value: Option<&Value>) -> bool {
    match value {
        None | Some(Value::Null) => false,
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Number(number)) => number.as_f64() != Some(0.0),
        Some(Value::String(text)) => !text.is_empty(),
        Some(_) => true,
    }
}
