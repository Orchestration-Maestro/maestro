// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 3] {
    [
        ("minimax.minimax-m2", minimax_dot_minimax_m2()),
        ("minimax.minimax-m2.1", minimax_dot_minimax_m2_dot_1()),
        ("minimax.minimax-m2.5", minimax_dot_minimax_m2_dot_5()),
    ]
}
fn minimax_dot_minimax_m2() -> Model {
    Model {
        id: "minimax.minimax-m2".into(),
        name: "MiniMax M2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_608.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn minimax_dot_minimax_m2_dot_1() -> Model {
    Model {
        id: "minimax.minimax-m2.1".into(),
        name: "MiniMax M2.1".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn minimax_dot_minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax.minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 98_304.0,
        headers: None,
        compat: None,
    }
}
