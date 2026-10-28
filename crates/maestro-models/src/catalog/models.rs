#![doc = include_str!("../../../../docs/models/catalog.md")]

use super::models_generated;
use crate::{Model, ModelThinkingLevel, Provider};
use ModelThinkingLevel::{High, Low, Medium, Minimal, Off, Xhigh};
use indexmap::IndexMap;
use std::sync::LazyLock;

/// Ordered provider registries keyed by exact model identifiers.
pub(super) type ModelRegistry = IndexMap<&'static str, IndexMap<&'static str, Model>>;
/// Lazily initialized catalog shared by descriptor lookups.
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

pub use maestro_request::types::calculate_cost;

/// Compare only provider and ID; either absent operand is unequal.
/// Any supplied descriptors may be compared, independently of catalog membership.
#[must_use]
pub fn models_are_equal(a: Option<&Model>, b: Option<&Model>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a.id == b.id && a.provider == b.provider)
}

/// Full ordered set of reasoning levels for extended-thinking models.
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
