// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_180() -> [(&'static str, Model); 10] {
    [
        (
            "qwen/qwen-2.5-72b-instruct",
            qwen_qwen_2_dot_5_72b_instruct(),
        ),
        ("qwen/qwen-2.5-7b-instruct", qwen_qwen_2_dot_5_7b_instruct()),
        ("qwen/qwen-max", qwen_qwen_max()),
        ("qwen/qwen-plus", qwen_qwen_plus()),
        ("qwen/qwen-plus-2025-07-28", qwen_qwen_plus_2025_07_28()),
        (
            "qwen/qwen-plus-2025-07-28:thinking",
            qwen_qwen_plus_2025_07_28_thinking(),
        ),
        ("qwen/qwen-turbo", qwen_qwen_turbo()),
        ("qwen/qwen-vl-max", qwen_qwen_vl_max()),
        ("qwen/qwen3-14b", qwen_qwen3_14b()),
        ("qwen/qwen3-235b-a22b", qwen_qwen3_235b_a22b()),
    ]
}
pub(super) fn models_190() -> [(&'static str, Model); 9] {
    [
        ("qwen/qwen3-235b-a22b-2507", qwen_qwen3_235b_a22b_2507()),
        (
            "qwen/qwen3-235b-a22b-thinking-2507",
            qwen_qwen3_235b_a22b_thinking_2507(),
        ),
        ("qwen/qwen3-30b-a3b", qwen_qwen3_30b_a3b()),
        (
            "qwen/qwen3-30b-a3b-instruct-2507",
            qwen_qwen3_30b_a3b_instruct_2507(),
        ),
        (
            "qwen/qwen3-30b-a3b-thinking-2507",
            qwen_qwen3_30b_a3b_thinking_2507(),
        ),
        ("qwen/qwen3-32b", qwen_qwen3_32b()),
        ("qwen/qwen3-8b", qwen_qwen3_8b()),
        ("qwen/qwen3-coder", qwen_qwen3_coder()),
        (
            "qwen/qwen3-coder-30b-a3b-instruct",
            qwen_qwen3_coder_30b_a3b_instruct(),
        ),
    ]
}
fn qwen_qwen_2_dot_5_72b_instruct() -> Model {
    Model {
        id: "qwen/qwen-2.5-72b-instruct".into(),
        name: "Qwen2.5 72B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.36,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen_2_dot_5_7b_instruct() -> Model {
    Model {
        id: "qwen/qwen-2.5-7b-instruct".into(),
        name: "Qwen: Qwen2.5 7B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.04,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen_max() -> Model {
    Model {
        id: "qwen/qwen-max".into(),
        name: "Qwen: Qwen-Max ".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.04,
            output: 4.16,
            cache_read: 0.208_000_000_000_000_02,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen_plus() -> Model {
    Model {
        id: "qwen/qwen-plus".into(),
        name: "Qwen: Qwen-Plus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.26,
            output: 0.78,
            cache_read: 0.052_000_000_000_000_005,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen_plus_2025_07_28() -> Model {
    Model {
        id: "qwen/qwen-plus-2025-07-28".into(),
        name: "Qwen: Qwen Plus 0728".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.26,
            output: 0.78,
            cache_read: 0.0,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen_plus_2025_07_28_thinking() -> Model {
    Model {
        id: "qwen/qwen-plus-2025-07-28:thinking".into(),
        name: "Qwen: Qwen Plus 0728 (thinking)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.26,
            output: 0.78,
            cache_read: 0.0,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen_turbo() -> Model {
    Model {
        id: "qwen/qwen-turbo".into(),
        name: "Qwen: Qwen-Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0325,
            output: 0.13,
            cache_read: 0.006_500_000_000_000_001,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen_vl_max() -> Model {
    Model {
        id: "qwen/qwen-vl-max".into(),
        name: "Qwen: Qwen VL Max".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.52,
            output: 2.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_14b() -> Model {
    Model {
        id: "qwen/qwen3-14b".into(),
        name: "Qwen: Qwen3 14B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 40_960.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_235b_a22b() -> Model {
    Model {
        id: "qwen/qwen3-235b-a22b".into(),
        name: "Qwen: Qwen3 235B A22B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.454_999_999_999_999_96,
            output: 1.819_999_999_999_999_8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_235b_a22b_2507() -> Model {
    Model {
        id: "qwen/qwen3-235b-a22b-2507".into(),
        name: "Qwen: Qwen3 235B A22B Instruct 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.071,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_235b_a22b_thinking_2507() -> Model {
    Model {
        id: "qwen/qwen3-235b-a22b-thinking-2507".into(),
        name: "Qwen: Qwen3 235B A22B Thinking 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.149_500_000_000_000_02,
            output: 1.495,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_30b_a3b() -> Model {
    Model {
        id: "qwen/qwen3-30b-a3b".into(),
        name: "Qwen: Qwen3 30B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 20_000.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_30b_a3b_instruct_2507() -> Model {
    Model {
        id: "qwen/qwen3-30b-a3b-instruct-2507".into(),
        name: "Qwen: Qwen3 30B A3B Instruct 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_30b_a3b_thinking_2507() -> Model {
    Model {
        id: "qwen/qwen3-30b-a3b-thinking-2507".into(),
        name: "Qwen: Qwen3 30B A3B Thinking 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_32b() -> Model {
    Model {
        id: "qwen/qwen3-32b".into(),
        name: "Qwen: Qwen3 32B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.24,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 40_960.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_8b() -> Model {
    Model {
        id: "qwen/qwen3-8b".into(),
        name: "Qwen: Qwen3 8B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_coder() -> Model {
    Model {
        id: "qwen/qwen3-coder".into(),
        name: "Qwen: Qwen3 Coder 480B A35B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 1.799_999_999_999_999_8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_coder_30b_a3b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-coder-30b-a3b-instruct".into(),
        name: "Qwen: Qwen3 Coder 30B A3B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.27,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 160_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
