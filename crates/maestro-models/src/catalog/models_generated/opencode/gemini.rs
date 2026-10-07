// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models_9() -> [(&'static str, Model); 2] {
    [
        ("gemini-3-flash", gemini_3_flash()),
        ("gemini-3.1-pro", gemini_3_dot_1_pro()),
    ]
}
fn gemini_3_flash() -> Model {
    Model {
        id: "gemini-3-flash".into(),
        name: "Gemini 3 Flash".into(),
        api: "google-generative-ai".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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

fn gemini_3_dot_1_pro() -> Model {
    Model {
        id: "gemini-3.1-pro".into(),
        name: "Gemini 3.1 Pro Preview".into(),
        api: "google-generative-ai".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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
