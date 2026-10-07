// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 3] {
    [
        ("gemini-3-pro-preview", gemini_3_pro_preview()),
        ("gemini-3.1-pro-preview", gemini_3_dot_1_pro_preview()),
        (
            "gemini-3.1-pro-preview-customtools",
            gemini_3_dot_1_pro_preview_customtools(),
        ),
    ]
}
fn gemini_3_pro_preview() -> Model {
    Model {
        id: "gemini-3-pro-preview".into(),
        name: "Gemini 3 Pro Preview (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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

fn gemini_3_dot_1_pro_preview() -> Model {
    Model {
        id: "gemini-3.1-pro-preview".into(),
        name: "Gemini 3.1 Pro Preview (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn gemini_3_dot_1_pro_preview_customtools() -> Model {
    Model {
        id: "gemini-3.1-pro-preview-customtools".into(),
        name: "Gemini 3.1 Pro Preview Custom Tools (Vertex)".into(),
        api: "google-vertex".into(),
        provider: "google-vertex".into(),
        base_url: "https://{location}-aiplatform.googleapis.com".into(),
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
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
