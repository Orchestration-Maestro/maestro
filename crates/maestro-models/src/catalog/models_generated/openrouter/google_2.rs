// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_270() -> [(&'static str, Model); 1] {
    [("~google/gemini-pro-latest", google_gemini_pro_latest())]
}
fn google_gemini_pro_latest() -> Model {
    Model {
        id: "~google/gemini-pro-latest".into(),
        name: "Google Gemini Pro Latest".into(),
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
