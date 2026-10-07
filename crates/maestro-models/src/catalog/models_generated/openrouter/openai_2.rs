// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_138() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-5-mini", openai_gpt_5_mini()),
        ("openai/gpt-5-nano", openai_gpt_5_nano()),
        ("openai/gpt-5-pro", openai_gpt_5_pro()),
        ("openai/gpt-5.1", openai_gpt_5_dot_1()),
        ("openai/gpt-5.1-chat", openai_gpt_5_dot_1_chat()),
        ("openai/gpt-5.1-codex", openai_gpt_5_dot_1_codex()),
        ("openai/gpt-5.1-codex-max", openai_gpt_5_dot_1_codex_max()),
        ("openai/gpt-5.1-codex-mini", openai_gpt_5_dot_1_codex_mini()),
        ("openai/gpt-5.2", openai_gpt_5_dot_2()),
        ("openai/gpt-5.2-chat", openai_gpt_5_dot_2_chat()),
    ]
}
pub(super) fn models_148() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-5.2-codex", openai_gpt_5_dot_2_codex()),
        ("openai/gpt-5.2-pro", openai_gpt_5_dot_2_pro()),
        ("openai/gpt-5.3-chat", openai_gpt_5_dot_3_chat()),
        ("openai/gpt-5.3-codex", openai_gpt_5_dot_3_codex()),
        ("openai/gpt-5.4", openai_gpt_5_dot_4()),
        ("openai/gpt-5.4-mini", openai_gpt_5_dot_4_mini()),
        ("openai/gpt-5.4-nano", openai_gpt_5_dot_4_nano()),
        ("openai/gpt-5.4-pro", openai_gpt_5_dot_4_pro()),
        ("openai/gpt-5.5", openai_gpt_5_dot_5()),
        ("openai/gpt-5.5-pro", openai_gpt_5_dot_5_pro()),
    ]
}
fn openai_gpt_5_mini() -> Model {
    Model {
        id: "openai/gpt-5-mini".into(),
        name: "OpenAI: GPT-5 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_nano() -> Model {
    Model {
        id: "openai/gpt-5-nano".into(),
        name: "OpenAI: GPT-5 Nano".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_pro() -> Model {
    Model {
        id: "openai/gpt-5-pro".into(),
        name: "OpenAI: GPT-5 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 120.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_1() -> Model {
    Model {
        id: "openai/gpt-5.1".into(),
        name: "OpenAI: GPT-5.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
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

fn openai_gpt_5_dot_1_chat() -> Model {
    Model {
        id: "openai/gpt-5.1-chat".into(),
        name: "OpenAI: GPT-5.1 Chat".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
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

fn openai_gpt_5_dot_1_codex() -> Model {
    Model {
        id: "openai/gpt-5.1-codex".into(),
        name: "OpenAI: GPT-5.1-Codex".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
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

fn openai_gpt_5_dot_1_codex_max() -> Model {
    Model {
        id: "openai/gpt-5.1-codex-max".into(),
        name: "OpenAI: GPT-5.1-Codex-Max".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
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

fn openai_gpt_5_dot_1_codex_mini() -> Model {
    Model {
        id: "openai/gpt-5.1-codex-mini".into(),
        name: "OpenAI: GPT-5.1-Codex-Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_2() -> Model {
    Model {
        id: "openai/gpt-5.2".into(),
        name: "OpenAI: GPT-5.2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_2_chat() -> Model {
    Model {
        id: "openai/gpt-5.2-chat".into(),
        name: "OpenAI: GPT-5.2 Chat".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_2_codex() -> Model {
    Model {
        id: "openai/gpt-5.2-codex".into(),
        name: "OpenAI: GPT-5.2-Codex".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_2_pro() -> Model {
    Model {
        id: "openai/gpt-5.2-pro".into(),
        name: "OpenAI: GPT-5.2 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 21.0,
            output: 168.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_3_chat() -> Model {
    Model {
        id: "openai/gpt-5.3-chat".into(),
        name: "OpenAI: GPT-5.3 Chat".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_3_codex() -> Model {
    Model {
        id: "openai/gpt-5.3-codex".into(),
        name: "OpenAI: GPT-5.3-Codex".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_4() -> Model {
    Model {
        id: "openai/gpt-5.4".into(),
        name: "OpenAI: GPT-5.4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_4_mini() -> Model {
    Model {
        id: "openai/gpt-5.4-mini".into(),
        name: "OpenAI: GPT-5.4 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.75,
            output: 4.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_4_nano() -> Model {
    Model {
        id: "openai/gpt-5.4-nano".into(),
        name: "OpenAI: GPT-5.4 Nano".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.25,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_4_pro() -> Model {
    Model {
        id: "openai/gpt-5.4-pro".into(),
        name: "OpenAI: GPT-5.4 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_5() -> Model {
    Model {
        id: "openai/gpt-5.5".into(),
        name: "OpenAI: GPT-5.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5_dot_5_pro() -> Model {
    Model {
        id: "openai/gpt-5.5-pro".into(),
        name: "OpenAI: GPT-5.5 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
