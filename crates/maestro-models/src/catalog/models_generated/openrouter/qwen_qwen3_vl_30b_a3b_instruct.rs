// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        (
            "qwen/qwen3-vl-30b-a3b-instruct",
            qwen_qwen3_vl_30b_a3b_instruct(),
        ),
        (
            "qwen/qwen3-vl-30b-a3b-thinking",
            qwen_qwen3_vl_30b_a3b_thinking(),
        ),
        ("qwen/qwen3-vl-32b-instruct", qwen_qwen3_vl_32b_instruct()),
        ("qwen/qwen3-vl-8b-instruct", qwen_qwen3_vl_8b_instruct()),
        ("qwen/qwen3-vl-8b-thinking", qwen_qwen3_vl_8b_thinking()),
        ("qwen/qwen3.5-122b-a10b", qwen_qwen3_dot_5_122b_a10b()),
        ("qwen/qwen3.5-27b", qwen_qwen3_dot_5_27b()),
        ("qwen/qwen3.5-35b-a3b", qwen_qwen3_dot_5_35b_a3b()),
        ("qwen/qwen3.5-397b-a17b", qwen_qwen3_dot_5_397b_a17b()),
        ("qwen/qwen3.5-9b", qwen_qwen3_dot_5_9b()),
    ]
}
fn qwen_qwen3_vl_30b_a3b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-30b-a3b-instruct".into(),
        name: "Qwen: Qwen3 VL 30B A3B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 0.52,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_vl_30b_a3b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-vl-30b-a3b-thinking".into(),
        name: "Qwen: Qwen3 VL 30B A3B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 1.56,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_vl_32b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-32b-instruct".into(),
        name: "Qwen: Qwen3 VL 32B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.104_000_000_000_000_01,
            output: 0.416_000_000_000_000_04,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_vl_8b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-8b-instruct".into(),
        name: "Qwen: Qwen3 VL 8B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.08,
            output: 0.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_vl_8b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-vl-8b-thinking".into(),
        name: "Qwen: Qwen3 VL 8B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.117,
            output: 1.365,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_122b_a10b() -> Model {
    Model {
        id: "qwen/qwen3.5-122b-a10b".into(),
        name: "Qwen: Qwen3.5-122B-A10B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.26,
            output: 2.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_27b() -> Model {
    Model {
        id: "qwen/qwen3.5-27b".into(),
        name: "Qwen: Qwen3.5-27B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.195,
            output: 1.56,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_35b_a3b() -> Model {
    Model {
        id: "qwen/qwen3.5-35b-a3b".into(),
        name: "Qwen: Qwen3.5-35B-A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 1.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_397b_a17b() -> Model {
    Model {
        id: "qwen/qwen3.5-397b-a17b".into(),
        name: "Qwen: Qwen3.5 397B A17B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.39,
            output: 2.34,
            cache_read: 0.195,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_9b() -> Model {
    Model {
        id: "qwen/qwen3.5-9b".into(),
        name: "Qwen: Qwen3.5-9B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}
