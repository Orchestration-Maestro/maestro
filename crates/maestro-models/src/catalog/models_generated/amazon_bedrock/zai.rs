// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_90() -> [(&'static str, Model); 3] {
    [
        ("zai.glm-4.7", zai_dot_glm_4_dot_7()),
        ("zai.glm-4.7-flash", zai_dot_glm_4_dot_7_flash()),
        ("zai.glm-5", zai_dot_glm_5()),
    ]
}
fn zai_dot_glm_4_dot_7() -> Model {
    Model {
        id: "zai.glm-4.7".into(),
        name: "GLM-4.7".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn zai_dot_glm_4_dot_7_flash() -> Model {
    Model {
        id: "zai.glm-4.7-flash".into(),
        name: "GLM-4.7-Flash".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn zai_dot_glm_5() -> Model {
    Model {
        id: "zai.glm-5".into(),
        name: "GLM-5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 101_376.0,
        headers: None,
        compat: None,
    }
}
