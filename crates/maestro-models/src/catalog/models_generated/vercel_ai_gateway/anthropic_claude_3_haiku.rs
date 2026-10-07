// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 10] {
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
            "anthropic/claude-haiku-4.5",
            anthropic_claude_haiku_4_dot_5(),
        ),
        ("anthropic/claude-opus-4", anthropic_claude_opus_4()),
        ("anthropic/claude-opus-4.1", anthropic_claude_opus_4_dot_1()),
        ("anthropic/claude-opus-4.5", anthropic_claude_opus_4_dot_5()),
        ("anthropic/claude-opus-4.6", anthropic_claude_opus_4_dot_6()),
        ("anthropic/claude-opus-4.7", anthropic_claude_opus_4_dot_7()),
        ("anthropic/claude-sonnet-4", anthropic_claude_sonnet_4()),
    ]
}
fn anthropic_claude_3_haiku() -> Model {
    Model {
        id: "anthropic/claude-3-haiku".into(),
        name: "Claude 3 Haiku".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_3_dot_5_haiku() -> Model {
    Model {
        id: "anthropic/claude-3.5-haiku".into(),
        name: "Claude 3.5 Haiku".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_3_dot_7_sonnet() -> Model {
    Model {
        id: "anthropic/claude-3.7-sonnet".into(),
        name: "Claude 3.7 Sonnet".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn anthropic_claude_haiku_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-haiku-4.5".into(),
        name: "Claude Haiku 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        name: "Claude Opus 4".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        name: "Claude Opus 4.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        name: "Claude Opus 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        name: "Claude Opus 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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

fn anthropic_claude_opus_4_dot_7() -> Model {
    Model {
        id: "anthropic/claude-opus-4.7".into(),
        name: "Claude Opus 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        name: "Claude Sonnet 4".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
