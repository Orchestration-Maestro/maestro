// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_228() -> [(&'static str, Model); 1] {
    [("rekaai/reka-edge", rekaai_reka_edge())]
}
fn rekaai_reka_edge() -> Model {
    Model {
        id: "rekaai/reka-edge".into(),
        name: "Reka Edge".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_384.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
