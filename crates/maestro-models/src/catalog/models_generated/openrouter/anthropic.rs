// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_8() -> [(&'static str, Model); 10] {
    [
        ("anthropic/claude-3-haiku", anthropic_claude_3_haiku()),
        (
            "anthropic/claude-3.5-haiku",
            anthropic_claude_3_dot_5_haiku(),
        ),
        (
            "anthropic/claude-3.7-sonnet",
            anthropic_claude_3_dot_7_sonnet(),
        ),
        (
            "anthropic/claude-3.7-sonnet:thinking",
            anthropic_claude_3_dot_7_sonnet_thinking(),
        ),
        (
            "anthropic/claude-haiku-4.5",
            anthropic_claude_haiku_4_dot_5(),
        ),
        ("anthropic/claude-opus-4", anthropic_claude_opus_4()),
        ("anthropic/claude-opus-4.1", anthropic_claude_opus_4_dot_1()),
        ("anthropic/claude-opus-4.5", anthropic_claude_opus_4_dot_5()),
        ("anthropic/claude-opus-4.6", anthropic_claude_opus_4_dot_6()),
        (
            "anthropic/claude-opus-4.6-fast",
            anthropic_claude_opus_4_dot_6_fast(),
        ),
    ]
}
pub(super) fn models_18() -> [(&'static str, Model); 4] {
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
pub(super) fn models_266() -> [(&'static str, Model); 3] {
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
fn anthropic_claude_3_haiku() -> Model {
    Model {
        id: "anthropic/claude-3-haiku".into(),
        name: "Anthropic: Claude 3 Haiku".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.25,
            cache_read: 0.03,
            cache_write: 0.3,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_3_dot_5_haiku() -> Model {
    Model {
        id: "anthropic/claude-3.5-haiku".into(),
        name: "Anthropic: Claude 3.5 Haiku".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.799_999_999_999_999_9,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        context_window: 200_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_3_dot_7_sonnet() -> Model {
    Model {
        id: "anthropic/claude-3.7-sonnet".into(),
        name: "Anthropic: Claude 3.7 Sonnet".into(),
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
        context_window: 200_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_3_dot_7_sonnet_thinking() -> Model {
    Model {
        id: "anthropic/claude-3.7-sonnet:thinking".into(),
        name: "Anthropic: Claude 3.7 Sonnet (thinking)".into(),
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
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_haiku_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-haiku-4.5".into(),
        name: "Anthropic: Claude Haiku 4.5".into(),
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

fn anthropic_claude_opus_4() -> Model {
    Model {
        id: "anthropic/claude-opus-4".into(),
        name: "Anthropic: Claude Opus 4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

fn anthropic_claude_opus_4_dot_1() -> Model {
    Model {
        id: "anthropic/claude-opus-4.1".into(),
        name: "Anthropic: Claude Opus 4.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

fn anthropic_claude_opus_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-opus-4.5".into(),
        name: "Anthropic: Claude Opus 4.5".into(),
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
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_opus_4_dot_6() -> Model {
    Model {
        id: "anthropic/claude-opus-4.6".into(),
        name: "Anthropic: Claude Opus 4.6".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

fn anthropic_claude_opus_4_dot_6_fast() -> Model {
    Model {
        id: "anthropic/claude-opus-4.6-fast".into(),
        name: "Anthropic: Claude Opus 4.6 (Fast)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 150.0,
            cache_read: 3.0,
            cache_write: 37.5,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
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
