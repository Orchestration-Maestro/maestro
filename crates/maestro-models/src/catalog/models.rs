#![doc = include_str!("../../../../docs/models/catalog.md")]

use super::models_generated;
use crate::{Model, ModelThinkingLevel, Provider, Usage, UsageCost};
use ModelThinkingLevel::{High, Low, Medium, Minimal, Off, Xhigh};
use indexmap::IndexMap;
use std::sync::LazyLock;

pub(super) type ModelRegistry = IndexMap<&'static str, IndexMap<&'static str, Model>>;
static MODEL_REGISTRY: LazyLock<ModelRegistry> = LazyLock::new(models_generated::models);

/// Look up an exact provider/model key, returning an independently owned descriptor.
/// Unknown keys return `None`; identifiers are never normalized.
#[must_use]
pub fn get_model(provider: &str, model_id: &str) -> Option<Model> {
    MODEL_REGISTRY.get(provider)?.get(model_id).cloned()
}

/// Return a fresh provider list in embedded insertion order.
#[must_use]
pub fn get_providers() -> Vec<Provider> {
    MODEL_REGISTRY
        .keys()
        .map(|provider| (*provider).into())
        .collect()
}

/// Return owned descriptors in embedded model order, or an empty list for unknown providers.
#[must_use]
pub fn get_models(provider: &str) -> Vec<Model> {
    MODEL_REGISTRY
        .get(provider)
        .map(|models| models.values().cloned().collect())
        .unwrap_or_default()
}

/// Update each reported category's flat-rate cost and their left-to-right sum.
/// Token counts are unchanged; the returned borrow is the supplied usage's cost record.
/// The descriptor need not belong to the catalog.
pub fn calculate_cost<'a>(model: &Model, usage: &'a mut Usage) -> &'a mut UsageCost {
    usage.cost.input = (model.cost.input / 1_000_000.0) * usage.input;
    usage.cost.output = (model.cost.output / 1_000_000.0) * usage.output;
    usage.cost.cache_read = (model.cost.cache_read / 1_000_000.0) * usage.cache_read;
    usage.cost.cache_write = (model.cost.cache_write / 1_000_000.0) * usage.cache_write;
    usage.cost.total =
        usage.cost.input + usage.cost.output + usage.cost.cache_read + usage.cost.cache_write;
    &mut usage.cost
}

/// Compare only provider and ID; either absent operand is unequal.
/// Any supplied descriptors may be compared, independently of catalog membership.
#[must_use]
pub fn models_are_equal(a: Option<&Model>, b: Option<&Model>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a.id == b.id && a.provider == b.provider)
}

const EXTENDED_THINKING_LEVELS: [ModelThinkingLevel; 6] = [Off, Minimal, Low, Medium, High, Xhigh];

/// Return supported levels in off/minimal/low/medium/high/xhigh order.
/// Nonreasoning descriptors support only off. Null disables a level; missing ordinary
/// mappings remain supported, while xhigh requires present text, including empty text.
/// The descriptor need not belong to the catalog.
#[must_use]
pub fn get_supported_thinking_levels(model: &Model) -> Vec<ModelThinkingLevel> {
    if !model.reasoning {
        return vec![Off];
    }
    EXTENDED_THINKING_LEVELS
        .into_iter()
        .filter(|level| {
            match model
                .thinking_level_map
                .as_ref()
                .and_then(|map| map.get(level))
            {
                Some(None) => false,
                Some(Some(_)) => true,
                None => *level != Xhigh,
            }
        })
        .collect()
}

/// Keep a supported request, otherwise prefer the next higher level, then the nearest lower.
/// Returns off if no level is available. Any supplied descriptor can be used.
#[must_use]
pub fn clamp_thinking_level(model: &Model, level: ModelThinkingLevel) -> ModelThinkingLevel {
    let available = get_supported_thinking_levels(model);
    if available.contains(&level) {
        return level;
    }
    available
        .iter()
        .find(|candidate| **candidate >= level)
        .or_else(|| available.last())
        .cloned()
        .unwrap_or(Off)
}
