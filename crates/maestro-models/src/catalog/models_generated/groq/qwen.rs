// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_16() -> [(&'static str, Model); 2] {
    [
        ("qwen-qwq-32b", qwen_qwq_32b()),
        ("qwen/qwen3-32b", qwen_qwen3_32b()),
    ]
}
fn qwen_qwq_32b() -> Model {
    Model {
        id: "qwen-qwq-32b".into(),
        name: "Qwen QwQ 32B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.29,
            output: 0.39,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_32b() -> Model {
    Model {
        id: "qwen/qwen3-32b".into(),
        name: "Qwen3 32B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, None),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("default".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.29,
            output: 0.59,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 40_960.0,
        headers: None,
        compat: None,
    }
}
