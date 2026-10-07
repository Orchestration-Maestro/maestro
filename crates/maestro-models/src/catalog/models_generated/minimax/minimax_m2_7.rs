// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        ("MiniMax-M2.7", minimax_m2_dot_7()),
        ("MiniMax-M2.7-highspeed", minimax_m2_dot_7_highspeed()),
    ]
}
fn minimax_m2_dot_7() -> Model {
    Model {
        id: "MiniMax-M2.7".into(),
        name: "MiniMax-M2.7".into(),
        api: "anthropic-messages".into(),
        provider: "minimax".into(),
        base_url: "https://api.minimax.io/anthropic".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn minimax_m2_dot_7_highspeed() -> Model {
    Model {
        id: "MiniMax-M2.7-highspeed".into(),
        name: "MiniMax-M2.7-highspeed".into(),
        api: "anthropic-messages".into(),
        provider: "minimax".into(),
        base_url: "https://api.minimax.io/anthropic".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
