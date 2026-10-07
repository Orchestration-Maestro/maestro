// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-4-turbo", openai_gpt_4_turbo()),
        ("openai/gpt-4.1", openai_gpt_4_dot_1()),
        ("openai/gpt-4.1-mini", openai_gpt_4_dot_1_mini()),
        ("openai/gpt-4.1-nano", openai_gpt_4_dot_1_nano()),
        ("openai/gpt-4o", openai_gpt_4o()),
        ("openai/gpt-4o-mini", openai_gpt_4o_mini()),
        ("openai/gpt-5", openai_gpt_5()),
        ("openai/gpt-5-chat", openai_gpt_5_chat()),
        ("openai/gpt-5-codex", openai_gpt_5_codex()),
        ("openai/gpt-5-mini", openai_gpt_5_mini()),
    ]
}
fn openai_gpt_4_turbo() -> Model {
    Model {
        id: "openai/gpt-4-turbo".into(),
        name: "GPT-4 Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 10.0,
            output: 30.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_dot_1() -> Model {
    Model {
        id: "openai/gpt-4.1".into(),
        name: "GPT-4.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_dot_1_mini() -> Model {
    Model {
        id: "openai/gpt-4.1-mini".into(),
        name: "GPT-4.1 mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 1.599_999_999_999_999_9,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4_dot_1_nano() -> Model {
    Model {
        id: "openai/gpt-4.1-nano".into(),
        name: "GPT-4.1 nano".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4o() -> Model {
    Model {
        id: "openai/gpt-4o".into(),
        name: "GPT-4o".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4o_mini() -> Model {
    Model {
        id: "openai/gpt-4o-mini".into(),
        name: "GPT-4o mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5() -> Model {
    Model {
        id: "openai/gpt-5".into(),
        name: "GPT-5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_chat() -> Model {
    Model {
        id: "openai/gpt-5-chat".into(),
        name: "GPT 5 Chat".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_codex() -> Model {
    Model {
        id: "openai/gpt-5-codex".into(),
        name: "GPT-5-Codex".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_mini() -> Model {
    Model {
        id: "openai/gpt-5-mini".into(),
        name: "GPT-5 mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
