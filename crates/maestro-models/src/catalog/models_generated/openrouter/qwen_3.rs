// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_218() -> [(&'static str, Model); 10] {
    [
        ("qwen/qwen3.5-397b-a17b", qwen_qwen3_dot_5_397b_a17b()),
        ("qwen/qwen3.5-9b", qwen_qwen3_dot_5_9b()),
        ("qwen/qwen3.5-flash-02-23", qwen_qwen3_dot_5_flash_02_23()),
        ("qwen/qwen3.5-plus-02-15", qwen_qwen3_dot_5_plus_02_15()),
        (
            "qwen/qwen3.5-plus-20260420",
            qwen_qwen3_dot_5_plus_20260420(),
        ),
        ("qwen/qwen3.6-27b", qwen_qwen3_dot_6_27b()),
        ("qwen/qwen3.6-35b-a3b", qwen_qwen3_dot_6_35b_a3b()),
        ("qwen/qwen3.6-flash", qwen_qwen3_dot_6_flash()),
        ("qwen/qwen3.6-max-preview", qwen_qwen3_dot_6_max_preview()),
        ("qwen/qwen3.6-plus", qwen_qwen3_dot_6_plus()),
    ]
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_flash_02_23() -> Model {
    Model {
        id: "qwen/qwen3.5-flash-02-23".into(),
        name: "Qwen: Qwen3.5-Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.065,
            output: 0.26,
            cache_read: 0.0,
            cache_write: 0.081_25,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_plus_02_15() -> Model {
    Model {
        id: "qwen/qwen3.5-plus-02-15".into(),
        name: "Qwen: Qwen3.5 Plus 2026-02-15".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.26,
            output: 1.56,
            cache_read: 0.0,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_5_plus_20260420() -> Model {
    Model {
        id: "qwen/qwen3.5-plus-20260420".into(),
        name: "Qwen: Qwen3.5 Plus 2026-04-20".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_6_27b() -> Model {
    Model {
        id: "qwen/qwen3.6-27b".into(),
        name: "Qwen: Qwen3.6 27B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.32,
            output: 3.199_999_999_999_999_7,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 81_920.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_6_35b_a3b() -> Model {
    Model {
        id: "qwen/qwen3.6-35b-a3b".into(),
        name: "Qwen: Qwen3.6 35B A3B".into(),
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

fn qwen_qwen3_dot_6_flash() -> Model {
    Model {
        id: "qwen/qwen3.6-flash".into(),
        name: "Qwen: Qwen3.6 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.3125,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_6_max_preview() -> Model {
    Model {
        id: "qwen/qwen3.6-max-preview".into(),
        name: "Qwen: Qwen3.6 Max Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.04,
            output: 6.24,
            cache_read: 0.0,
            cache_write: 1.3,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn qwen_qwen3_dot_6_plus() -> Model {
    Model {
        id: "qwen/qwen3.6-plus".into(),
        name: "Qwen: Qwen3.6 Plus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.325,
            output: 1.95,
            cache_read: 0.0,
            cache_write: 0.406_25,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
