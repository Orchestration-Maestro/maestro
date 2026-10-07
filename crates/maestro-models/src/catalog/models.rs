use super::generated::MODELS;
use crate::{Model, Provider};
use std::sync::{Arc, LazyLock, RwLock};
type Entries = Vec<(String, Arc<RwLock<Model>>)>;
static REGISTRY: LazyLock<Vec<(Provider, Entries)>> = LazyLock::new(|| {
    let providers: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(MODELS).expect("embedded catalog JSON");
    providers
        .into_iter()
        .map(|(provider, models)| {
            let models = models
                .as_object()
                .expect("embedded provider object")
                .iter()
                .map(|(id, descriptor)| {
                    let model = serde_json::from_value(descriptor.clone())
                        .expect("embedded model descriptor");
                    (id.clone(), Arc::new(RwLock::new(model)))
                })
                .collect();
            (provider, models)
        })
        .collect()
});
/// Returns the process-lived shared descriptor at the exact original keys.
/// Missing providers or identifiers return `None`; inputs are not normalized.
/// Descriptor edits are visible through every handle without changing lookup keys.
pub fn get_model(provider: &str, model_id: &str) -> Option<Arc<RwLock<Model>>> {
    REGISTRY
        .iter()
        .find(|(key, _)| key == provider)?
        .1
        .iter()
        .find(|(key, _)| key == model_id)
        .map(|(_, model)| Arc::clone(model))
}
/// Returns a fresh list of shared descriptors in generated insertion order.
/// Unknown providers return an empty list. Handles retain process-lived identity.
pub fn get_models(provider: &str) -> Vec<Arc<RwLock<Model>>> {
    REGISTRY
        .iter()
        .find(|(key, _)| key == provider)
        .map(|(_, models)| models.iter().map(|(_, model)| Arc::clone(model)).collect())
        .unwrap_or_default()
}
/// Returns a fresh provider-key list in generated insertion order.
/// Keys remain independent of descriptor edits; enumeration performs no I/O.
pub fn get_providers() -> Vec<Provider> {
    REGISTRY
        .iter()
        .map(|(provider, _)| provider.clone())
        .collect()
}
/// Overwrites the supplied usage costs with USD-per-million rates.
/// Divides each rate before multiplying tokens, then sums in category order.
/// Returns the same mutable cost record; token counts and numeric edge values
/// are neither validated nor rounded. The model need not belong to the catalog.
pub fn calculate_cost<'a>(model: &Model, usage: &'a mut crate::Usage) -> &'a mut crate::UsageCost {
    usage.cost.input = (model.cost.input / 1000000.0) * usage.input;
    usage.cost.output = (model.cost.output / 1000000.0) * usage.output;
    usage.cost.cache_read = (model.cost.cache_read / 1000000.0) * usage.cache_read;
    usage.cost.cache_write = (model.cost.cache_write / 1000000.0) * usage.cache_write;
    usage.cost.total =
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
    &mut usage.cost
}
/// Compares only exact provider and id strings of supplied descriptors.
/// Returns false when either operand is absent, including two absent operands.
/// Other fields and catalog membership have no effect.
pub fn models_are_equal(a: Option<&Model>, b: Option<&Model>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.id == b.id && a.provider == b.provider,
        _ => false,
    }
}
const EXTENDED_LEVELS: [(&str, crate::ModelThinkingLevel); 6] = [
    ("off", crate::ModelThinkingLevel::Off),
    ("minimal", crate::ModelThinkingLevel::Minimal),
    ("low", crate::ModelThinkingLevel::Low),
    ("medium", crate::ModelThinkingLevel::Medium),
    ("high", crate::ModelThinkingLevel::High),
    ("xhigh", crate::ModelThinkingLevel::Xhigh),
];
/// Returns a fresh ordered off/minimal/low/medium/high/xhigh capability list.
/// Nonreasoning models return only off. Explicit null removes any mapped level;
/// xhigh requires a present non-null mapping. Other mapped values are not read
/// as capability settings, and missing ordinary mappings retain their levels.
pub fn get_supported_thinking_levels(model: &Model) -> Vec<crate::ModelThinkingLevel> {
    if !model.reasoning {
        return vec![crate::ModelThinkingLevel::Off];
    }
    EXTENDED_LEVELS
        .iter()
        .filter(|(name, _)| {
            let mapped = model
                .thinking_level_map
                .as_ref()
                .and_then(|map| map.get(*name));
            if mapped.is_some_and(serde_json::Value::is_null) {
                return false;
            }
            *name != "xhigh" || mapped.is_some()
        })
        .map(|(_, level)| level.clone())
        .collect()
}
/// Clamps an exact off/minimal/low/medium/high/xhigh spelling to a supported level.
/// Supported requests return unchanged; unavailable levels search upward before
/// downward. Unknown strings use the first available level, with off as the
/// empty-list fallback. No trimming, case folding or model mutation occurs.
pub fn clamp_thinking_level(model: &Model, level: &str) -> crate::ModelThinkingLevel {
    let available = get_supported_thinking_levels(model);
    if let Some(index) = EXTENDED_LEVELS.iter().position(|(name, _)| *name == level) {
        for (_, candidate) in EXTENDED_LEVELS[index..]
            .iter()
            .chain(EXTENDED_LEVELS[..index].iter().rev())
        {
            if available.contains(candidate) {
                return candidate.clone();
            }
        }
    }
    available
        .into_iter()
        .next()
        .unwrap_or(crate::ModelThinkingLevel::Off)
}
