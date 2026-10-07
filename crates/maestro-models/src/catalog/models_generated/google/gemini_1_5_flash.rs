// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
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
        max_tokens: 8_192.0,
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
            input: 0.037_5,
            output: 0.15,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8_192.0,
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
            cache_read: 0.312_5,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
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
        max_tokens: 8_192.0,
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
            cache_read: 0.037_5,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
