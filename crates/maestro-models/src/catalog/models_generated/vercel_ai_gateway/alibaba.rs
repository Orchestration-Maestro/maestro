// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 10] {
    [
        ("alibaba/qwen-3-14b", alibaba_qwen_3_14b()),
        ("alibaba/qwen-3-235b", alibaba_qwen_3_235b()),
        ("alibaba/qwen-3-30b", alibaba_qwen_3_30b()),
        ("alibaba/qwen-3-32b", alibaba_qwen_3_32b()),
        (
            "alibaba/qwen-3.6-max-preview",
            alibaba_qwen_3_dot_6_max_preview(),
        ),
        (
            "alibaba/qwen3-235b-a22b-thinking",
            alibaba_qwen3_235b_a22b_thinking(),
        ),
        ("alibaba/qwen3-coder", alibaba_qwen3_coder()),
        ("alibaba/qwen3-coder-30b-a3b", alibaba_qwen3_coder_30b_a3b()),
        ("alibaba/qwen3-coder-next", alibaba_qwen3_coder_next()),
        ("alibaba/qwen3-coder-plus", alibaba_qwen3_coder_plus()),
    ]
}
pub(super) fn models_10() -> [(&'static str, Model); 8] {
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
fn alibaba_qwen_3_14b() -> Model {
    Model {
        id: "alibaba/qwen-3-14b".into(),
        name: "Qwen3-14B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.12,
            output: 0.24,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen_3_235b() -> Model {
    Model {
        id: "alibaba/qwen-3-235b".into(),
        name: "Qwen3 235B A22b Instruct 2507".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 1.2,
            cache_read: 0.6,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen_3_30b() -> Model {
    Model {
        id: "alibaba/qwen-3-30b".into(),
        name: "Qwen3-30B-A3B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.29,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen_3_32b() -> Model {
    Model {
        id: "alibaba/qwen-3-32b".into(),
        name: "Qwen 3 32B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.16,
            output: 0.64,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen_3_dot_6_max_preview() -> Model {
    Model {
        id: "alibaba/qwen-3.6-max-preview".into(),
        name: "Qwen 3.6 Max Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.3,
            output: 7.8,
            cache_read: 0.26,
            cache_write: 1.625,
        },
        context_window: 240_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_235b_a22b_thinking() -> Model {
    Model {
        id: "alibaba/qwen3-235b-a22b-thinking".into(),
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

fn alibaba_qwen3_coder() -> Model {
    Model {
        id: "alibaba/qwen3-coder".into(),
        name: "Qwen3 Coder 480B A35B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.5,
            output: 7.5,
            cache_read: 0.3,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_coder_30b_a3b() -> Model {
    Model {
        id: "alibaba/qwen3-coder-30b-a3b".into(),
        name: "Qwen 3 Coder 30B A3B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_coder_next() -> Model {
    Model {
        id: "alibaba/qwen3-coder-next".into(),
        name: "Qwen3 Coder Next".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn alibaba_qwen3_coder_plus() -> Model {
    Model {
        id: "alibaba/qwen3-coder-plus".into(),
        name: "Qwen3 Coder Plus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
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
