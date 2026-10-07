// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
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
            "gemini-2.5-flash-lite-preview-09-2025",
            gemini_2_dot_5_flash_lite_preview_09_2025(),
        ),
        ("gemini-2.5-pro", gemini_2_dot_5_pro()),
        ("gemini-3-flash-preview", gemini_3_flash_preview()),
    ]
}
fn gemini_1_dot_5_flash() -> Model {
    Model {
        id: "gemini-1.5-flash".into(),
        name: "Gemini 1.5 Flash (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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
        name: "Gemini 1.5 Flash-8B (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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
        name: "Gemini 1.5 Pro (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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
        name: "Gemini 2.0 Flash (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.037_5,
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
        name: "Gemini 2.0 Flash Lite (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.018_75,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_2_dot_5_flash() -> Model {
    Model {
        id: "gemini-2.5-flash".into(),
        name: "Gemini 2.5 Flash (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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
        name: "Gemini 2.5 Flash Lite (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.01,
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
        name: "Gemini 2.5 Flash Lite Preview 09-25 (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.4,
            cache_read: 0.01,
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
        name: "Gemini 2.5 Pro (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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

fn gemini_3_flash_preview() -> Model {
    Model {
        id: "gemini-3-flash-preview".into(),
        name: "Gemini 3 Flash Preview (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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
