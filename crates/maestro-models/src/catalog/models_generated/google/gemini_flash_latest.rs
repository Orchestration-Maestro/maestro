// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 4] {
    [
        ("gemini-flash-latest", gemini_flash_latest()),
        ("gemini-flash-lite-latest", gemini_flash_lite_latest()),
        ("gemini-live-2.5-flash", gemini_live_2_dot_5_flash()),
        (
            "gemini-live-2.5-flash-preview-native-audio",
            gemini_live_2_dot_5_flash_preview_native_audio(),
        ),
    ]
}
fn gemini_flash_latest() -> Model {
    Model {
        id: "gemini-flash-latest".into(),
        name: "Gemini Flash Latest".into(),
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

fn gemini_flash_lite_latest() -> Model {
    Model {
        id: "gemini-flash-lite-latest".into(),
        name: "Gemini Flash-Lite Latest".into(),
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

fn gemini_live_2_dot_5_flash() -> Model {
    Model {
        id: "gemini-live-2.5-flash".into(),
        name: "Gemini Live 2.5 Flash".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_000.0,
        headers: None,
        compat: None,
    }
}

fn gemini_live_2_dot_5_flash_preview_native_audio() -> Model {
    Model {
        id: "gemini-live-2.5-flash-preview-native-audio".into(),
        name: "Gemini Live 2.5 Flash Preview Native Audio".into(),
        api: "google-generative-ai".into(),
        provider: "google".into(),
        base_url: "https://generativelanguage.googleapis.com/v1beta".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
