// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 3] {
    [
        ("z-ai/glm-5-turbo", z_ai_glm_5_turbo()),
        ("z-ai/glm-5.1", z_ai_glm_5_dot_1()),
        ("z-ai/glm-5v-turbo", z_ai_glm_5v_turbo()),
    ]
}
fn z_ai_glm_5_turbo() -> Model {
    Model {
        id: "z-ai/glm-5-turbo".into(),
        name: "Z.ai: GLM 5 Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_5_dot_1() -> Model {
    Model {
        id: "z-ai/glm-5.1".into(),
        name: "Z.ai: GLM 5.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.049_999_999_999_999_8,
            output: 3.5,
            cache_read: 0.524_999_999_999_999_9,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_5v_turbo() -> Model {
    Model {
        id: "z-ai/glm-5v-turbo".into(),
        name: "Z.ai: GLM 5V Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
