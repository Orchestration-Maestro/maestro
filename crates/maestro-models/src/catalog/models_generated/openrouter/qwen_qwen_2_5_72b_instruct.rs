// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
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
        max_tokens: 8_192.0,
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
            input: 0.032_5,
            output: 0.13,
            cache_read: 0.006_500_000_000_000_001,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}
