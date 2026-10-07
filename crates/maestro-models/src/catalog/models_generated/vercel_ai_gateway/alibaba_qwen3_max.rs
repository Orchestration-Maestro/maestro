// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 8] {
    [
        ("alibaba/qwen3-max", alibaba_qwen3_max()),
        ("alibaba/qwen3-max-preview", alibaba_qwen3_max_preview()),
        ("alibaba/qwen3-max-thinking", alibaba_qwen3_max_thinking()),
        ("alibaba/qwen3-vl-thinking", alibaba_qwen3_vl_thinking()),
        ("alibaba/qwen3.5-flash", alibaba_qwen3_dot_5_flash()),
        ("alibaba/qwen3.5-plus", alibaba_qwen3_dot_5_plus()),
        ("alibaba/qwen3.6-27b", alibaba_qwen3_dot_6_27b()),
        ("alibaba/qwen3.6-plus", alibaba_qwen3_dot_6_plus()),
    ]
}
fn alibaba_qwen3_max() -> Model {
    Model {
        id: "alibaba/qwen3-max".into(),
        name: "Qwen3 Max".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 6.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_max_preview() -> Model {
    Model {
        id: "alibaba/qwen3-max-preview".into(),
        name: "Qwen3 Max Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 6.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_max_thinking() -> Model {
    Model {
        id: "alibaba/qwen3-max-thinking".into(),
        name: "Qwen 3 Max Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 6.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_vl_thinking() -> Model {
    Model {
        id: "alibaba/qwen3-vl-thinking".into(),
        name: "Qwen3 VL 235B A22B Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 4.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_dot_5_flash() -> Model {
    Model {
        id: "alibaba/qwen3.5-flash".into(),
        name: "Qwen 3.5 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.001,
            cache_write: 0.125,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_dot_5_plus() -> Model {
    Model {
        id: "alibaba/qwen3.5-plus".into(),
        name: "Qwen 3.5 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.4,
            cache_read: 0.04,
            cache_write: 0.5,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_dot_6_27b() -> Model {
    Model {
        id: "alibaba/qwen3.6-27b".into(),
        name: "Qwen 3.6 27B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.599_999_999_999_999_6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_dot_6_plus() -> Model {
    Model {
        id: "alibaba/qwen3.6-plus".into(),
        name: "Qwen 3.6 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 0.625,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}
