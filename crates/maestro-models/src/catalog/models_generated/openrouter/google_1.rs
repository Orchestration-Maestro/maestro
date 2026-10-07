// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_46() -> [(&'static str, Model); 10] {
    [
        (
            "google/gemini-2.0-flash-001",
            google_gemini_2_dot_0_flash_001(),
        ),
        (
            "google/gemini-2.0-flash-lite-001",
            google_gemini_2_dot_0_flash_lite_001(),
        ),
        ("google/gemini-2.5-flash", google_gemini_2_dot_5_flash()),
        (
            "google/gemini-2.5-flash-lite",
            google_gemini_2_dot_5_flash_lite(),
        ),
        (
            "google/gemini-2.5-flash-lite-preview-09-2025",
            google_gemini_2_dot_5_flash_lite_preview_09_2025(),
        ),
        ("google/gemini-2.5-pro", google_gemini_2_dot_5_pro()),
        (
            "google/gemini-2.5-pro-preview",
            google_gemini_2_dot_5_pro_preview(),
        ),
        (
            "google/gemini-2.5-pro-preview-05-06",
            google_gemini_2_dot_5_pro_preview_05_06(),
        ),
        (
            "google/gemini-3-flash-preview",
            google_gemini_3_flash_preview(),
        ),
        (
            "google/gemini-3.1-flash-lite-preview",
            google_gemini_3_dot_1_flash_lite_preview(),
        ),
    ]
}
pub(super) fn models_56() -> [(&'static str, Model); 8] {
    [
        (
            "google/gemini-3.1-pro-preview",
            google_gemini_3_dot_1_pro_preview(),
        ),
        (
            "google/gemini-3.1-pro-preview-customtools",
            google_gemini_3_dot_1_pro_preview_customtools(),
        ),
        ("google/gemma-3-12b-it", google_gemma_3_12b_it()),
        ("google/gemma-3-27b-it", google_gemma_3_27b_it()),
        ("google/gemma-4-26b-a4b-it", google_gemma_4_26b_a4b_it()),
        (
            "google/gemma-4-26b-a4b-it:free",
            google_gemma_4_26b_a4b_it_free(),
        ),
        ("google/gemma-4-31b-it", google_gemma_4_31b_it()),
        ("google/gemma-4-31b-it:free", google_gemma_4_31b_it_free()),
    ]
}
pub(super) fn models_269() -> [(&'static str, Model); 1] {
    [("~google/gemini-flash-latest", google_gemini_flash_latest())]
}
fn google_gemini_2_dot_0_flash_001() -> Model {
    Model {
        id: "google/gemini-2.0-flash-001".into(),
        name: "Google: Gemini 2.0 Flash".into(),
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
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_0_flash_lite_001() -> Model {
    Model {
        id: "google/gemini-2.0-flash-lite-001".into(),
        name: "Google: Gemini 2.0 Flash Lite".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_flash() -> Model {
    Model {
        id: "google/gemini-2.5-flash".into(),
        name: "Google: Gemini 2.5 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_flash_lite() -> Model {
    Model {
        id: "google/gemini-2.5-flash-lite".into(),
        name: "Google: Gemini 2.5 Flash Lite".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_flash_lite_preview_09_2025() -> Model {
    Model {
        id: "google/gemini-2.5-flash-lite-preview-09-2025".into(),
        name: "Google: Gemini 2.5 Flash Lite Preview 09-2025".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_pro() -> Model {
    Model {
        id: "google/gemini-2.5-pro".into(),
        name: "Google: Gemini 2.5 Pro".into(),
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
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_pro_preview() -> Model {
    Model {
        id: "google/gemini-2.5-pro-preview".into(),
        name: "Google: Gemini 2.5 Pro Preview 06-05".into(),
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
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_2_dot_5_pro_preview_05_06() -> Model {
    Model {
        id: "google/gemini-2.5-pro-preview-05-06".into(),
        name: "Google: Gemini 2.5 Pro Preview 05-06".into(),
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
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_flash_preview() -> Model {
    Model {
        id: "google/gemini-3-flash-preview".into(),
        name: "Google: Gemini 3 Flash Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_dot_1_flash_lite_preview() -> Model {
    Model {
        id: "google/gemini-3.1-flash-lite-preview".into(),
        name: "Google: Gemini 3.1 Flash Lite Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_dot_1_pro_preview() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview".into(),
        name: "Google: Gemini 3.1 Pro Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_3_dot_1_pro_preview_customtools() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview-customtools".into(),
        name: "Google: Gemini 3.1 Pro Preview Custom Tools".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_3_12b_it() -> Model {
    Model {
        id: "google/gemma-3-12b-it".into(),
        name: "Google: Gemma 3 12B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.04,
            output: 0.13,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_3_27b_it() -> Model {
    Model {
        id: "google/gemma-3-27b-it".into(),
        name: "Google: Gemma 3 27B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.08,
            output: 0.16,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_26b_a4b_it() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it".into(),
        name: "Google: Gemma 4 26B A4B ".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.06,
            output: 0.33,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_26b_a4b_it_free() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it:free".into(),
        name: "Google: Gemma 4 26B A4B  (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_31b_it() -> Model {
    Model {
        id: "google/gemma-4-31b-it".into(),
        name: "Google: Gemma 4 31B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 0.38,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn google_gemma_4_31b_it_free() -> Model {
    Model {
        id: "google/gemma-4-31b-it:free".into(),
        name: "Google: Gemma 4 31B (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn google_gemini_flash_latest() -> Model {
    Model {
        id: "~google/gemini-flash-latest".into(),
        name: "Google Gemini Flash Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
