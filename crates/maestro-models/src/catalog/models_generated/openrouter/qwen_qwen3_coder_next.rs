// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("qwen/qwen3-coder-next", qwen_qwen3_coder_next()),
        ("qwen/qwen3-coder-plus", qwen_qwen3_coder_plus()),
        ("qwen/qwen3-coder:free", qwen_qwen3_coder_free()),
        ("qwen/qwen3-max", qwen_qwen3_max()),
        ("qwen/qwen3-max-thinking", qwen_qwen3_max_thinking()),
        (
            "qwen/qwen3-next-80b-a3b-instruct",
            qwen_qwen3_next_80b_a3b_instruct(),
        ),
        (
            "qwen/qwen3-next-80b-a3b-instruct:free",
            qwen_qwen3_next_80b_a3b_instruct_free(),
        ),
        (
            "qwen/qwen3-next-80b-a3b-thinking",
            qwen_qwen3_next_80b_a3b_thinking(),
        ),
        (
            "qwen/qwen3-vl-235b-a22b-instruct",
            qwen_qwen3_vl_235b_a22b_instruct(),
        ),
        (
            "qwen/qwen3-vl-235b-a22b-thinking",
            qwen_qwen3_vl_235b_a22b_thinking(),
        ),
    ]
}
fn qwen_qwen3_coder_next() -> Model {
    Model {
        id: "qwen/qwen3-coder-next".into(),
        name: "Qwen: Qwen3 Coder Next".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.12,
            output: 0.799_999_999_999_999_9,
            cache_read: 0.07,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_coder_plus() -> Model {
    Model {
        id: "qwen/qwen3-coder-plus".into(),
        name: "Qwen: Qwen3 Coder Plus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.65,
            output: 3.25,
            cache_read: 0.13,
            cache_write: 0.812_5,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_coder_free() -> Model {
    Model {
        id: "qwen/qwen3-coder:free".into(),
        name: "Qwen: Qwen3 Coder 480B A35B (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_max() -> Model {
    Model {
        id: "qwen/qwen3-max".into(),
        name: "Qwen: Qwen3 Max".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.78,
            output: 3.9,
            cache_read: 0.156,
            cache_write: 0.975,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_max_thinking() -> Model {
    Model {
        id: "qwen/qwen3-max-thinking".into(),
        name: "Qwen: Qwen3 Max Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.78,
            output: 3.9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_next_80b_a3b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-next-80b-a3b-instruct".into(),
        name: "Qwen: Qwen3 Next 80B A3B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 1.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_next_80b_a3b_instruct_free() -> Model {
    Model {
        id: "qwen/qwen3-next-80b-a3b-instruct:free".into(),
        name: "Qwen: Qwen3 Next 80B A3B Instruct (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_next_80b_a3b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-next-80b-a3b-thinking".into(),
        name: "Qwen: Qwen3 Next 80B A3B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.097_5,
            output: 0.78,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_vl_235b_a22b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-235b-a22b-instruct".into(),
        name: "Qwen: Qwen3 VL 235B A22B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.88,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_vl_235b_a22b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-vl-235b-a22b-thinking".into(),
        name: "Qwen: Qwen3 VL 235B A22B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.26,
            output: 2.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
