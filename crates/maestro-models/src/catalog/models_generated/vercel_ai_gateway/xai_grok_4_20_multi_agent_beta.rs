// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 7] {
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
