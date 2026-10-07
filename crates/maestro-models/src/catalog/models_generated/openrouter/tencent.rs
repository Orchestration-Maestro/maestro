// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_233() -> [(&'static str, Model); 1] {
    [("tencent/hy3-preview:free", tencent_hy3_preview_free())]
}
fn tencent_hy3_preview_free() -> Model {
    Model {
        id: "tencent/hy3-preview:free".into(),
        name: "Tencent: Hy3 preview (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}
