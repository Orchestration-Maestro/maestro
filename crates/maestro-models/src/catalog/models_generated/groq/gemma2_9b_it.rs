// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [("gemma2-9b-it", gemma2_9b_it())]
}
fn gemma2_9b_it() -> Model {
    Model {
        id: "gemma2-9b-it".into(),
        name: "Gemma 2 9B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_192.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}
