// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 4] {
    [
        ("anthropic/claude-opus-4.7", anthropic_claude_opus_4_dot_7()),
        ("anthropic/claude-sonnet-4", anthropic_claude_sonnet_4()),
        (
            "anthropic/claude-sonnet-4.5",
            anthropic_claude_sonnet_4_dot_5(),
        ),
        (
            "anthropic/claude-sonnet-4.6",
            anthropic_claude_sonnet_4_dot_6(),
        ),
    ]
}
fn anthropic_claude_opus_4_dot_7() -> Model {
    Model {
        id: "anthropic/claude-opus-4.7".into(),
        name: "Anthropic: Claude Opus 4.7".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
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

fn anthropic_claude_sonnet_4() -> Model {
    Model {
        id: "anthropic/claude-sonnet-4".into(),
        name: "Anthropic: Claude Sonnet 4".into(),
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
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_sonnet_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-sonnet-4.5".into(),
        name: "Anthropic: Claude Sonnet 4.5".into(),
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
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_sonnet_4_dot_6() -> Model {
    Model {
        id: "anthropic/claude-sonnet-4.6".into(),
        name: "Anthropic: Claude Sonnet 4.6".into(),
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
