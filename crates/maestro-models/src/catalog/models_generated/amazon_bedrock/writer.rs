// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_88() -> [(&'static str, Model); 2] {
    [
        ("writer.palmyra-x4-v1:0", writer_dot_palmyra_x4_v1_0()),
        ("writer.palmyra-x5-v1:0", writer_dot_palmyra_x5_v1_0()),
    ]
}
fn writer_dot_palmyra_x4_v1_0() -> Model {
    Model {
        id: "writer.palmyra-x4-v1:0".into(),
        name: "Palmyra X4".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 122_880.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn writer_dot_palmyra_x5_v1_0() -> Model {
    Model {
        id: "writer.palmyra-x5-v1:0".into(),
        name: "Palmyra X5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_040_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}
