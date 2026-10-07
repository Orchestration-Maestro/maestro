// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_128() -> [(&'static str, Model); 10] {
    [
        ("xai/grok-3", xai_grok_3()),
        ("xai/grok-3-fast", xai_grok_3_fast()),
        ("xai/grok-3-mini", xai_grok_3_mini()),
        ("xai/grok-3-mini-fast", xai_grok_3_mini_fast()),
        ("xai/grok-4", xai_grok_4()),
        (
            "xai/grok-4-fast-non-reasoning",
            xai_grok_4_fast_non_reasoning(),
        ),
        ("xai/grok-4-fast-reasoning", xai_grok_4_fast_reasoning()),
        (
            "xai/grok-4.1-fast-non-reasoning",
            xai_grok_4_dot_1_fast_non_reasoning(),
        ),
        (
            "xai/grok-4.1-fast-reasoning",
            xai_grok_4_dot_1_fast_reasoning(),
        ),
        ("xai/grok-4.20-multi-agent", xai_grok_4_dot_20_multi_agent()),
    ]
}
pub(super) fn models_138() -> [(&'static str, Model); 7] {
    [
        (
            "xai/grok-4.20-multi-agent-beta",
            xai_grok_4_dot_20_multi_agent_beta(),
        ),
        (
            "xai/grok-4.20-non-reasoning",
            xai_grok_4_dot_20_non_reasoning(),
        ),
        (
            "xai/grok-4.20-non-reasoning-beta",
            xai_grok_4_dot_20_non_reasoning_beta(),
        ),
        ("xai/grok-4.20-reasoning", xai_grok_4_dot_20_reasoning()),
        (
            "xai/grok-4.20-reasoning-beta",
            xai_grok_4_dot_20_reasoning_beta(),
        ),
        ("xai/grok-4.3", xai_grok_4_dot_3()),
        ("xai/grok-code-fast-1", xai_grok_code_fast_1()),
    ]
}
fn xai_grok_3() -> Model {
    Model {
        id: "xai/grok-3".into(),
        name: "Grok 3 Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_3_fast() -> Model {
    Model {
        id: "xai/grok-3-fast".into(),
        name: "Grok 3 Fast Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_3_mini() -> Model {
    Model {
        id: "xai/grok-3-mini".into(),
        name: "Grok 3 Mini Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_3_mini_fast() -> Model {
    Model {
        id: "xai/grok-3-mini-fast".into(),
        name: "Grok 3 Mini Fast Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 4.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4() -> Model {
    Model {
        id: "xai/grok-4".into(),
        name: "Grok 4".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_fast_non_reasoning() -> Model {
    Model {
        id: "xai/grok-4-fast-non-reasoning".into(),
        name: "Grok 4 Fast Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_fast_reasoning() -> Model {
    Model {
        id: "xai/grok-4-fast-reasoning".into(),
        name: "Grok 4 Fast Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_1_fast_non_reasoning() -> Model {
    Model {
        id: "xai/grok-4.1-fast-non-reasoning".into(),
        name: "Grok 4.1 Fast Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_1_fast_reasoning() -> Model {
    Model {
        id: "xai/grok-4.1-fast-reasoning".into(),
        name: "Grok 4.1 Fast Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_20_multi_agent() -> Model {
    Model {
        id: "xai/grok-4.20-multi-agent".into(),
        name: "Grok 4.20 Multi-Agent".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_20_multi_agent_beta() -> Model {
    Model {
        id: "xai/grok-4.20-multi-agent-beta".into(),
        name: "Grok 4.20 Multi Agent Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_20_non_reasoning() -> Model {
    Model {
        id: "xai/grok-4.20-non-reasoning".into(),
        name: "Grok 4.20 Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_20_non_reasoning_beta() -> Model {
    Model {
        id: "xai/grok-4.20-non-reasoning-beta".into(),
        name: "Grok 4.20 Beta Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_20_reasoning() -> Model {
    Model {
        id: "xai/grok-4.20-reasoning".into(),
        name: "Grok 4.20 Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_20_reasoning_beta() -> Model {
    Model {
        id: "xai/grok-4.20-reasoning-beta".into(),
        name: "Grok 4.20 Beta Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_4_dot_3() -> Model {
    Model {
        id: "xai/grok-4.3".into(),
        name: "Grok 4.3".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 2.5,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 1_000_000.0,
        headers: None,
        compat: None,
    }
}

fn xai_grok_code_fast_1() -> Model {
    Model {
        id: "xai/grok-code-fast-1".into(),
        name: "Grok Code Fast 1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.5,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}
