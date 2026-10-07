// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-4.1-nano", openai_gpt_4_dot_1_nano()),
        ("openai/gpt-4o", openai_gpt_4o()),
        ("openai/gpt-4o-2024-05-13", openai_gpt_4o_2024_05_13()),
        ("openai/gpt-4o-2024-08-06", openai_gpt_4o_2024_08_06()),
        ("openai/gpt-4o-2024-11-20", openai_gpt_4o_2024_11_20()),
        ("openai/gpt-4o-audio-preview", openai_gpt_4o_audio_preview()),
        ("openai/gpt-4o-mini", openai_gpt_4o_mini()),
        (
            "openai/gpt-4o-mini-2024-07-18",
            openai_gpt_4o_mini_2024_07_18(),
        ),
        ("openai/gpt-5", openai_gpt_5()),
        ("openai/gpt-5-codex", openai_gpt_5_codex()),
    ]
}
fn openai_gpt_4_dot_1_nano() -> Model {
    Model {
        id: "openai/gpt-4.1-nano".into(),
        name: "OpenAI: GPT-4.1 Nano".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4o() -> Model {
    Model {
        id: "openai/gpt-4o".into(),
        name: "OpenAI: GPT-4o".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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

fn openai_gpt_4o_2024_05_13() -> Model {
    Model {
        id: "openai/gpt-4o-2024-05-13".into(),
        name: "OpenAI: GPT-4o (2024-05-13)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4o_2024_08_06() -> Model {
    Model {
        id: "openai/gpt-4o-2024-08-06".into(),
        name: "OpenAI: GPT-4o (2024-08-06)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

fn openai_gpt_4o_2024_11_20() -> Model {
    Model {
        id: "openai/gpt-4o-2024-11-20".into(),
        name: "OpenAI: GPT-4o (2024-11-20)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

fn openai_gpt_4o_audio_preview() -> Model {
    Model {
        id: "openai/gpt-4o-audio-preview".into(),
        name: "OpenAI: GPT-4o Audio".into(),
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

fn openai_gpt_4o_mini() -> Model {
    Model {
        id: "openai/gpt-4o-mini".into(),
        name: "OpenAI: GPT-4o-mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_4o_mini_2024_07_18() -> Model {
    Model {
        id: "openai/gpt-4o-mini-2024-07-18".into(),
        name: "OpenAI: GPT-4o-mini (2024-07-18)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn openai_gpt_5() -> Model {
    Model {
        id: "openai/gpt-5".into(),
        name: "OpenAI: GPT-5".into(),
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

fn openai_gpt_5_codex() -> Model {
    Model {
        id: "openai/gpt-5-codex".into(),
        name: "OpenAI: GPT-5 Codex".into(),
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
