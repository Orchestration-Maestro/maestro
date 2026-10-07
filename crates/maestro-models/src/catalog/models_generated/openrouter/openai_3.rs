// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_158() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-audio", openai_gpt_audio()),
        ("openai/gpt-audio-mini", openai_gpt_audio_mini()),
        ("openai/gpt-oss-120b", openai_gpt_oss_120b()),
        ("openai/gpt-oss-120b:free", openai_gpt_oss_120b_free()),
        ("openai/gpt-oss-20b", openai_gpt_oss_20b()),
        ("openai/gpt-oss-20b:free", openai_gpt_oss_20b_free()),
        (
            "openai/gpt-oss-safeguard-20b",
            openai_gpt_oss_safeguard_20b(),
        ),
        ("openai/o1", openai_o1()),
        ("openai/o3", openai_o3()),
        ("openai/o3-deep-research", openai_o3_deep_research()),
    ]
}
pub(super) fn models_168() -> [(&'static str, Model); 6] {
    [
        ("openai/o3-mini", openai_o3_mini()),
        ("openai/o3-mini-high", openai_o3_mini_high()),
        ("openai/o3-pro", openai_o3_pro()),
        ("openai/o4-mini", openai_o4_mini()),
        (
            "openai/o4-mini-deep-research",
            openai_o4_mini_deep_research(),
        ),
        ("openai/o4-mini-high", openai_o4_mini_high()),
    ]
}
pub(super) fn models_272() -> [(&'static str, Model); 2] {
    [
        ("~openai/gpt-latest", openai_gpt_latest()),
        ("~openai/gpt-mini-latest", openai_gpt_mini_latest()),
    ]
}
fn openai_gpt_audio() -> Model {
    Model {
        id: "openai/gpt-audio".into(),
        name: "OpenAI: GPT Audio".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_audio_mini() -> Model {
    Model {
        id: "openai/gpt-audio-mini".into(),
        name: "OpenAI: GPT Audio Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_oss_120b() -> Model {
    Model {
        id: "openai/gpt-oss-120b".into(),
        name: "OpenAI: gpt-oss-120b".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.039,
            output: 0.18,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_oss_120b_free() -> Model {
    Model {
        id: "openai/gpt-oss-120b:free".into(),
        name: "OpenAI: gpt-oss-120b (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_oss_20b() -> Model {
    Model {
        id: "openai/gpt-oss-20b".into(),
        name: "OpenAI: gpt-oss-20b".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.03,
            output: 0.14,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_oss_20b_free() -> Model {
    Model {
        id: "openai/gpt-oss-20b:free".into(),
        name: "OpenAI: gpt-oss-20b (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_oss_safeguard_20b() -> Model {
    Model {
        id: "openai/gpt-oss-safeguard-20b".into(),
        name: "OpenAI: gpt-oss-safeguard-20b".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.037,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn openai_o1() -> Model {
    Model {
        id: "openai/o1".into(),
        name: "OpenAI: o1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 60.0,
            cache_read: 7.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3() -> Model {
    Model {
        id: "openai/o3".into(),
        name: "OpenAI: o3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3_deep_research() -> Model {
    Model {
        id: "openai/o3-deep-research".into(),
        name: "OpenAI: o3 Deep Research".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 10.0,
            output: 40.0,
            cache_read: 2.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3_mini() -> Model {
    Model {
        id: "openai/o3-mini".into(),
        name: "OpenAI: o3 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.55,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3_mini_high() -> Model {
    Model {
        id: "openai/o3-mini-high".into(),
        name: "OpenAI: o3 Mini High".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.55,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o3_pro() -> Model {
    Model {
        id: "openai/o3-pro".into(),
        name: "OpenAI: o3 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 20.0,
            output: 80.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o4_mini() -> Model {
    Model {
        id: "openai/o4-mini".into(),
        name: "OpenAI: o4 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.275,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o4_mini_deep_research() -> Model {
    Model {
        id: "openai/o4-mini-deep-research".into(),
        name: "OpenAI: o4 Mini Deep Research".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_o4_mini_high() -> Model {
    Model {
        id: "openai/o4-mini-high".into(),
        name: "OpenAI: o4 Mini High".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.275,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_latest() -> Model {
    Model {
        id: "~openai/gpt-latest".into(),
        name: "OpenAI GPT Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
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

fn openai_gpt_mini_latest() -> Model {
    Model {
        id: "~openai/gpt-mini-latest".into(),
        name: "OpenAI GPT Mini Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
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
