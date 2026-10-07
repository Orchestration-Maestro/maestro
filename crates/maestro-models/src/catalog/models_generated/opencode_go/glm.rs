// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_2() -> [(&'static str, Model); 2] {
    [("glm-5", glm_5()), ("glm-5.1", glm_5_dot_1())]
}
fn glm_5() -> Model {
    Model {
        id: "glm-5".into(),
        name: "GLM-5".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.2,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn glm_5_dot_1() -> Model {
    Model {
        id: "glm-5.1".into(),
        name: "GLM-5.1".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
