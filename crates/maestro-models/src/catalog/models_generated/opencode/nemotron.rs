// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_35() -> [(&'static str, Model); 1] {
    [("nemotron-3-super-free", nemotron_3_super_free())]
}
fn nemotron_3_super_free() -> Model {
    Model {
        id: "nemotron-3-super-free".into(),
        name: "Nemotron 3 Super Free".into(),
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
        context_window: 204_800.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
