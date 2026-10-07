// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_0() -> [(&'static str, Model); 10] {
    [
        ("gemini-1.5-flash", gemini_1_dot_5_flash()),
        ("gemini-1.5-flash-8b", gemini_1_dot_5_flash_8b()),
        ("gemini-1.5-pro", gemini_1_dot_5_pro()),
        ("gemini-2.0-flash", gemini_2_dot_0_flash()),
        ("gemini-2.0-flash-lite", gemini_2_dot_0_flash_lite()),
        ("gemini-2.5-flash", gemini_2_dot_5_flash()),
        ("gemini-2.5-flash-lite", gemini_2_dot_5_flash_lite()),
        (
            "gemini-2.5-flash-lite-preview-06-17",
            gemini_2_dot_5_flash_lite_preview_06_17(),
        ),
        (
            "gemini-2.5-flash-lite-preview-09-2025",
            gemini_2_dot_5_flash_lite_preview_09_2025(),
        ),
        (
            "gemini-2.5-flash-preview-04-17",
            gemini_2_dot_5_flash_preview_04_17(),
        ),
    ]
}
pub(super) fn models_10() -> [(&'static str, Model); 8] {
    [
        (
            "gemini-2.5-flash-preview-05-20",
            gemini_2_dot_5_flash_preview_05_20(),
        ),
        (
            "gemini-2.5-flash-preview-09-2025",
            gemini_2_dot_5_flash_preview_09_2025(),
        ),
        ("gemini-2.5-pro", gemini_2_dot_5_pro()),
        (
            "gemini-2.5-pro-preview-05-06",
            gemini_2_dot_5_pro_preview_05_06(),
        ),
        (
            "gemini-2.5-pro-preview-06-05",
            gemini_2_dot_5_pro_preview_06_05(),
        ),
        ("gemini-3-flash-preview", gemini_3_flash_preview()),
        ("gemini-3-pro-preview", gemini_3_pro_preview()),
        (
            "gemini-3.1-flash-lite-preview",
            gemini_3_dot_1_flash_lite_preview(),
        ),
    ]
}
fn gemini_1_dot_5_flash() -> Model {
    Model {
        id: "gemini-1.5-flash".into(),
        name: "Gemini 1.5 Flash".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.018_75,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn gemini_1_dot_5_flash_8b() -> Model {
    Model {
        id: "gemini-1.5-flash-8b".into(),
        name: "Gemini 1.5 Flash-8B".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0375,
            output: 0.15,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn gemini_1_dot_5_pro() -> Model {
    Model {
        id: "gemini-1.5-pro".into(),
        name: "Gemini 1.5 Pro".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 5.0,
            cache_read: 0.3125,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_0_flash() -> Model {
    Model {
        id: "gemini-2.0-flash".into(),
        name: "Gemini 2.0 Flash".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_0_flash_lite() -> Model {
    Model {
        id: "gemini-2.0-flash-lite".into(),
        name: "Gemini 2.0 Flash Lite".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
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

fn gemini_2_dot_5_flash() -> Model {
    Model {
        id: "gemini-2.5-flash".into(),
        name: "Gemini 2.5 Flash".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_flash_lite() -> Model {
    Model {
        id: "gemini-2.5-flash-lite".into(),
        name: "Gemini 2.5 Flash Lite".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_flash_lite_preview_06_17() -> Model {
    Model {
        id: "gemini-2.5-flash-lite-preview-06-17".into(),
        name: "Gemini 2.5 Flash Lite Preview 06-17".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_flash_lite_preview_09_2025() -> Model {
    Model {
        id: "gemini-2.5-flash-lite-preview-09-2025".into(),
        name: "Gemini 2.5 Flash Lite Preview 09-25".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_flash_preview_04_17() -> Model {
    Model {
        id: "gemini-2.5-flash-preview-04-17".into(),
        name: "Gemini 2.5 Flash Preview 04-17".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0375,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_flash_preview_05_20() -> Model {
    Model {
        id: "gemini-2.5-flash-preview-05-20".into(),
        name: "Gemini 2.5 Flash Preview 05-20".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0375,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_flash_preview_09_2025() -> Model {
    Model {
        id: "gemini-2.5-flash-preview-09-2025".into(),
        name: "Gemini 2.5 Flash Preview 09-25".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_pro() -> Model {
    Model {
        id: "gemini-2.5-pro".into(),
        name: "Gemini 2.5 Pro".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_pro_preview_05_06() -> Model {
    Model {
        id: "gemini-2.5-pro-preview-05-06".into(),
        name: "Gemini 2.5 Pro Preview 05-06".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.31,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_pro_preview_06_05() -> Model {
    Model {
        id: "gemini-2.5-pro-preview-06-05".into(),
        name: "Gemini 2.5 Pro Preview 06-05".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.31,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_3_flash_preview() -> Model {
    Model {
        id: "gemini-3-flash-preview".into(),
        name: "Gemini 3 Flash Preview".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_3_pro_preview() -> Model {
    Model {
        id: "gemini-3-pro-preview".into(),
        name: "Gemini 3 Pro Preview".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, Some("LOW".into())),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("HIGH".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn gemini_3_dot_1_flash_lite_preview() -> Model {
    Model {
        id: "gemini-3.1-flash-lite-preview".into(),
        name: "Gemini 3.1 Flash Lite Preview".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.025,
            cache_write: 1.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
