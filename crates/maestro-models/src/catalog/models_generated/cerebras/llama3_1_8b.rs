// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [("llama3.1-8b", llama3_dot_1_8b())]
}
fn llama3_dot_1_8b() -> Model {
    Model {
        id: "llama3.1-8b".into(),
        name: "Llama 3.1 8B".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 8_000.0,
        headers: None,
        compat: None,
    }
}
