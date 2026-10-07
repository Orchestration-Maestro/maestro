// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_3() -> [(&'static str, Model); 1] {
    [("zai-glm-4.7", zai_glm_4_dot_7())]
}
fn zai_glm_4_dot_7() -> Model {
    Model {
        id: "zai-glm-4.7".into(),
        name: "Z.AI GLM-4.7".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.25,
            output: 2.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}
