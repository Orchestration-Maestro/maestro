// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        ("minimax-m2.5", minimax_m2_dot_5()),
        ("minimax-m2.7", minimax_m2_dot_7()),
    ]
}
fn minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

fn minimax_m2_dot_7() -> Model {
    Model {
        id: "minimax-m2.7".into(),
        name: "MiniMax M2.7".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
