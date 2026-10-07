// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_1() -> [(&'static str, Model); 8] {
    [
        ("claude-haiku-4-5", claude_haiku_4_5()),
        ("claude-opus-4-1", claude_opus_4_1()),
        ("claude-opus-4-5", claude_opus_4_5()),
        ("claude-opus-4-6", claude_opus_4_6()),
        ("claude-opus-4-7", claude_opus_4_7()),
        ("claude-sonnet-4", claude_sonnet_4()),
        ("claude-sonnet-4-5", claude_sonnet_4_5()),
        ("claude-sonnet-4-6", claude_sonnet_4_6()),
    ]
}
fn claude_haiku_4_5() -> Model {
    Model {
        id: "claude-haiku-4-5".into(),
        name: "Claude Haiku 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_1() -> Model {
    Model {
        id: "claude-opus-4-1".into(),
        name: "Claude Opus 4.1".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_5() -> Model {
    Model {
        id: "claude-opus-4-5".into(),
        name: "Claude Opus 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_opus_4_6() -> Model {
    Model {
        id: "claude-opus-4-6".into(),
        name: "Claude Opus 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
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

fn claude_opus_4_7() -> Model {
    Model {
        id: "claude-opus-4-7".into(),
        name: "Claude Opus 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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

fn claude_sonnet_4() -> Model {
    Model {
        id: "claude-sonnet-4".into(),
        name: "Claude Sonnet 4".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_sonnet_4_5() -> Model {
    Model {
        id: "claude-sonnet-4-5".into(),
        name: "Claude Sonnet 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn claude_sonnet_4_6() -> Model {
    Model {
        id: "claude-sonnet-4-6".into(),
        name: "Claude Sonnet 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
