// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 3] {
    [
        (
            "~anthropic/claude-haiku-latest",
            anthropic_claude_haiku_latest(),
        ),
        (
            "~anthropic/claude-opus-latest",
            anthropic_claude_opus_latest(),
        ),
        (
            "~anthropic/claude-sonnet-latest",
            anthropic_claude_sonnet_latest(),
        ),
    ]
}
fn anthropic_claude_haiku_latest() -> Model {
    Model {
        id: "~anthropic/claude-haiku-latest".into(),
        name: "Anthropic Claude Haiku Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 1.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_opus_latest() -> Model {
    Model {
        id: "~anthropic/claude-opus-latest".into(),
        name: "Anthropic: Claude Opus Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_sonnet_latest() -> Model {
    Model {
        id: "~anthropic/claude-sonnet-latest".into(),
        name: "Anthropic Claude Sonnet Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
