// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
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
        ("qwen/qwen3-coder-flash", qwen_qwen3_coder_flash()),
    ]
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
        max_tokens: 4_096.0,
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
        max_tokens: 8_192.0,
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

fn qwen_qwen3_coder_flash() -> Model {
    Model {
        id: "qwen/qwen3-coder-flash".into(),
        name: "Qwen: Qwen3 Coder Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.195,
            output: 0.975,
            cache_read: 0.039,
            cache_write: 0.243_75,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
