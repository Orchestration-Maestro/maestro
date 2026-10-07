// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [(
        "deepseek-r1-distill-llama-70b",
        deepseek_r1_distill_llama_70b(),
    )]
}
fn deepseek_r1_distill_llama_70b() -> Model {
    Model {
        id: "deepseek-r1-distill-llama-70b".into(),
        name: "DeepSeek R1 Distill Llama 70B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.75,
            output: 0.99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}
