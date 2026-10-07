// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_17() -> [(&'static str, Model); 1] {
    [(
        "accounts/fireworks/models/qwen3p6-plus",
        accounts_fireworks_models_qwen3p6_plus(),
    )]
}
fn accounts_fireworks_models_qwen3p6_plus() -> Model {
    Model {
        id: "accounts/fireworks/models/qwen3p6-plus".into(),
        name: "Qwen 3.6 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}
