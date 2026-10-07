// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_21() -> [(&'static str, Model); 3] {
    [
        ("deepseek.r1-v1:0", deepseek_dot_r1_v1_0()),
        ("deepseek.v3-v1:0", deepseek_dot_v3_v1_0()),
        ("deepseek.v3.2", deepseek_dot_v3_dot_2()),
    ]
}
fn deepseek_dot_r1_v1_0() -> Model {
    Model {
        id: "deepseek.r1-v1:0".into(),
        name: "DeepSeek-R1".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.35,
            output: 5.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_dot_v3_v1_0() -> Model {
    Model {
        id: "deepseek.v3-v1:0".into(),
        name: "DeepSeek-V3.1".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.58,
            output: 1.68,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 81_920.0,
        headers: None,
        compat: None,
    }
}

fn deepseek_dot_v3_dot_2() -> Model {
    Model {
        id: "deepseek.v3.2".into(),
        name: "DeepSeek-V3.2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.62,
            output: 1.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 81_920.0,
        headers: None,
        compat: None,
    }
}
