// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        ("moonshotai/kimi-k2-instruct", moonshotai_kimi_k2_instruct()),
        (
            "moonshotai/kimi-k2-instruct-0905",
            moonshotai_kimi_k2_instruct_0905(),
        ),
    ]
}
fn moonshotai_kimi_k2_instruct() -> Model {
    Model {
        id: "moonshotai/kimi-k2-instruct".into(),
        name: "Kimi K2 Instruct".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn moonshotai_kimi_k2_instruct_0905() -> Model {
    Model {
        id: "moonshotai/kimi-k2-instruct-0905".into(),
        name: "Kimi K2 Instruct 0905".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
