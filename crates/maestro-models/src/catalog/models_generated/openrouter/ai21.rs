// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [("ai21/jamba-large-1.7", ai21_jamba_large_1_dot_7())]
}
fn ai21_jamba_large_1_dot_7() -> Model {
    Model {
        id: "ai21/jamba-large-1.7".into(),
        name: "AI21: Jamba Large 1.7".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
