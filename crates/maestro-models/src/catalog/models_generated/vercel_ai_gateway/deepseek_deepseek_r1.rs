// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 8] {
    [
        ("deepseek/deepseek-r1", deepseek_deepseek_r1()),
        ("deepseek/deepseek-v3", deepseek_deepseek_v3()),
        ("deepseek/deepseek-v3.1", deepseek_deepseek_v3_dot_1()),
        (
            "deepseek/deepseek-v3.1-terminus",
            deepseek_deepseek_v3_dot_1_terminus(),
        ),
        ("deepseek/deepseek-v3.2", deepseek_deepseek_v3_dot_2()),
        (
            "deepseek/deepseek-v3.2-thinking",
            deepseek_deepseek_v3_dot_2_thinking(),
        ),
        ("deepseek/deepseek-v4-flash", deepseek_deepseek_v4_flash()),
        ("deepseek/deepseek-v4-pro", deepseek_deepseek_v4_pro()),
    ]
}
fn deepseek_deepseek_r1() -> Model {
    Model {
        id: "deepseek/deepseek-r1".into(),
        name: "DeepSeek-R1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.35,
            output: 5.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3() -> Model {
    Model {
        id: "deepseek/deepseek-v3".into(),
        name: "DeepSeek V3 0324".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.77,
            output: 0.77,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3_dot_1() -> Model {
    Model {
        id: "deepseek/deepseek-v3.1".into(),
        name: "DeepSeek-V3.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.56,
            output: 1.68,
            cache_read: 0.28,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3_dot_1_terminus() -> Model {
    Model {
        id: "deepseek/deepseek-v3.1-terminus".into(),
        name: "DeepSeek V3.1 Terminus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.27,
            output: 1.0,
            cache_read: 0.135,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3_dot_2() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2".into(),
        name: "DeepSeek V3.2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.28,
            output: 0.42,
            cache_read: 0.028,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_000.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v3_dot_2_thinking() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2-thinking".into(),
        name: "DeepSeek V3.2 Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.62,
            output: 1.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_000.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v4_flash() -> Model {
    Model {
        id: "deepseek/deepseek-v4-flash".into(),
        name: "DeepSeek V4 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.002_8,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 384_000.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek/deepseek-v4-pro".into(),
        name: "DeepSeek V4 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.003_6,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 384_000.0,
        headers: None,
        compat: None,
    }
}
