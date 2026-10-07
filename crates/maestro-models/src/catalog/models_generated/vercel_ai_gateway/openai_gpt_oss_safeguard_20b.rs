// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 7] {
    [
        (
            "openai/gpt-oss-safeguard-20b",
            openai_gpt_oss_safeguard_20b(),
        ),
        ("openai/o1", openai_o1()),
        ("openai/o3", openai_o3()),
        ("openai/o3-deep-research", openai_o3_deep_research()),
        ("openai/o3-mini", openai_o3_mini()),
        ("openai/o3-pro", openai_o3_pro()),
        ("openai/o4-mini", openai_o4_mini()),
    ]
}
fn openai_gpt_oss_safeguard_20b() -> Model {
    Model {
        id: "openai/gpt-oss-safeguard-20b".into(),
        name: "GPT OSS Safeguard 20B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.037,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn openai_o1() -> Model {
    Model {
        id: "openai/o1".into(),
        name: "o1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 60.0,
            cache_read: 7.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3() -> Model {
    Model {
        id: "openai/o3".into(),
        name: "o3".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3_deep_research() -> Model {
    Model {
        id: "openai/o3-deep-research".into(),
        name: "o3-deep-research".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 10.0,
            output: 40.0,
            cache_read: 2.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3_mini() -> Model {
    Model {
        id: "openai/o3-mini".into(),
        name: "o3-mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.55,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3_pro() -> Model {
    Model {
        id: "openai/o3-pro".into(),
        name: "o3 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 20.0,
            output: 80.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o4_mini() -> Model {
    Model {
        id: "openai/o4-mini".into(),
        name: "o4-mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.275,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}
