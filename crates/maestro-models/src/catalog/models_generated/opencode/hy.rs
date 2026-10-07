// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_29() -> [(&'static str, Model); 1] {
    [("hy3-preview-free", hy3_preview_free())]
}
fn hy3_preview_free() -> Model {
    Model {
        id: "hy3-preview-free".into(),
        name: "Hy3 preview Free".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}
