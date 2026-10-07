// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat, ThinkingFormat};
pub(super) fn models_12() -> [(&'static str, Model); 2] {
    [
        ("qwen3.5-plus", qwen3_dot_5_plus()),
        ("qwen3.6-plus", qwen3_dot_6_plus()),
    ]
}
fn qwen3_dot_5_plus() -> Model {
    Model {
        id: "qwen3.5-plus".into(),
        name: "Qwen3.5 Plus".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 1.2,
            cache_read: 0.02,
            cache_write: 0.25,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                thinking_format: Some(ThinkingFormat::Qwen),
                ..Default::default()
            },
        ))),
    }
}

fn qwen3_dot_6_plus() -> Model {
    Model {
        id: "qwen3.6-plus".into(),
        name: "Qwen3.6 Plus".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.625,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                thinking_format: Some(ThinkingFormat::Qwen),
                ..Default::default()
            },
        ))),
    }
}
