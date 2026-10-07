// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_0() -> [(&'static str, Model); 10] {
    [
        ("gpt-4", gpt_4()),
        ("gpt-4-turbo", gpt_4_turbo()),
        ("gpt-4.1", gpt_4_dot_1()),
        ("gpt-4.1-mini", gpt_4_dot_1_mini()),
        ("gpt-4.1-nano", gpt_4_dot_1_nano()),
        ("gpt-4o", gpt_4o()),
        ("gpt-4o-2024-05-13", gpt_4o_2024_05_13()),
        ("gpt-4o-2024-08-06", gpt_4o_2024_08_06()),
        ("gpt-4o-2024-11-20", gpt_4o_2024_11_20()),
        ("gpt-4o-mini", gpt_4o_mini()),
    ]
}
pub(super) fn models_10() -> [(&'static str, Model); 10] {
    [
        ("gpt-5", gpt_5()),
        ("gpt-5-chat-latest", gpt_5_chat_latest()),
        ("gpt-5-codex", gpt_5_codex()),
        ("gpt-5-mini", gpt_5_mini()),
        ("gpt-5-nano", gpt_5_nano()),
        ("gpt-5-pro", gpt_5_pro()),
        ("gpt-5.1", gpt_5_dot_1()),
        ("gpt-5.1-chat-latest", gpt_5_dot_1_chat_latest()),
        ("gpt-5.1-codex", gpt_5_dot_1_codex()),
        ("gpt-5.1-codex-max", gpt_5_dot_1_codex_max()),
    ]
}
fn gpt_4() -> Model {
    Model {
        id: "gpt-4".into(),
        name: "GPT-4".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 30.0,
            output: 60.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn gpt_4_turbo() -> Model {
    Model {
        id: "gpt-4-turbo".into(),
        name: "GPT-4 Turbo".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn gpt_4_dot_1() -> Model {
    Model {
        id: "gpt-4.1".into(),
        name: "GPT-4.1".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
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

fn gpt_4_dot_1_mini() -> Model {
    Model {
        id: "gpt-4.1-mini".into(),
        name: "GPT-4.1 mini".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.4,
            output: 1.6,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn gpt_4_dot_1_nano() -> Model {
    Model {
        id: "gpt-4.1-nano".into(),
        name: "GPT-4.1 nano".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn gpt_4o() -> Model {
    Model {
        id: "gpt-4o".into(),
        name: "GPT-4o".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
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

fn gpt_4o_2024_05_13() -> Model {
    Model {
        id: "gpt-4o-2024-05-13".into(),
        name: "GPT-4o (2024-05-13)".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 15.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn gpt_4o_2024_08_06() -> Model {
    Model {
        id: "gpt-4o-2024-08-06".into(),
        name: "GPT-4o (2024-08-06)".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
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

fn gpt_4o_2024_11_20() -> Model {
    Model {
        id: "gpt-4o-2024-11-20".into(),
        name: "GPT-4o (2024-11-20)".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
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

fn gpt_4o_mini() -> Model {
    Model {
        id: "gpt-4o-mini".into(),
        name: "GPT-4o mini".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5() -> Model {
    Model {
        id: "gpt-5".into(),
        name: "GPT-5".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
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

fn gpt_5_chat_latest() -> Model {
    Model {
        id: "gpt-5-chat-latest".into(),
        name: "GPT-5 Chat Latest".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
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

fn gpt_5_codex() -> Model {
    Model {
        id: "gpt-5-codex".into(),
        name: "GPT-5-Codex".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
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

fn gpt_5_mini() -> Model {
    Model {
        id: "gpt-5-mini".into(),
        name: "GPT-5 Mini".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_nano() -> Model {
    Model {
        id: "gpt-5-nano".into(),
        name: "GPT-5 Nano".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.05,
            output: 0.4,
            cache_read: 0.005,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_pro() -> Model {
    Model {
        id: "gpt-5-pro".into(),
        name: "GPT-5 Pro".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 120.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 272_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_1() -> Model {
    Model {
        id: "gpt-5.1".into(),
        name: "GPT-5.1".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.13,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn gpt_5_dot_1_chat_latest() -> Model {
    Model {
        id: "gpt-5.1-chat-latest".into(),
        name: "GPT-5.1 Chat".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
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

fn gpt_5_dot_1_codex() -> Model {
    Model {
        id: "gpt-5.1-codex".into(),
        name: "GPT-5.1 Codex".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
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

fn gpt_5_dot_1_codex_max() -> Model {
    Model {
        id: "gpt-5.1-codex-max".into(),
        name: "GPT-5.1 Codex Max".into(),
        api: "azure-openai-responses".into(),
        provider: "azure-openai-responses".into(),
        base_url: String::new(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
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
