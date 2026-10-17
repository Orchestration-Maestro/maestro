//! Shared model descriptor for context witnesses.
#![cfg(test)]
use std::sync::{Arc, RwLock};
/// A model descriptor with the supplied identity.
pub fn model(id: &str) -> Arc<RwLock<maestro_models::Model>> {
    Arc::new(RwLock::new(maestro_models::Model {
        id: id.into(),
        name: id.into(),
        api: "fixture".into(),
        provider: "fixture".into(),
        base_url: "https://example.invalid".into(),
        reasoning: false,
        thinking_level_map: None,
        input: Vec::new(),
        cost: maestro_models::ModelCost::default(),
        context_window: 200_000.0,
        max_tokens: 20.0,
        headers: None,
        compat: None,
    }))
}
