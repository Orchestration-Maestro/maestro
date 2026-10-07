// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 3] {
    [
        ("zai/glm-5-turbo", zai_glm_5_turbo()),
        ("zai/glm-5.1", zai_glm_5_dot_1()),
        ("zai/glm-5v-turbo", zai_glm_5v_turbo()),
    ]
}
fn zai_glm_5_turbo() -> Model {
    Model {
        id: "zai/glm-5-turbo".into(),
        name: "GLM 5 Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 202_800.0,
        max_tokens: 131_100.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_5_dot_1() -> Model {
    Model {
        id: "zai/glm-5.1".into(),
        name: "GLM 5.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 202_800.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_5v_turbo() -> Model {
    Model {
        id: "zai/glm-5v-turbo".into(),
        name: "GLM 5V Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
