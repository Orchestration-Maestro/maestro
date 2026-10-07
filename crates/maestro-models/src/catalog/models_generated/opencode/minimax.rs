// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_32() -> [(&'static str, Model); 3] {
    [
        ("minimax-m2.5", minimax_m2_dot_5()),
        ("minimax-m2.5-free", minimax_m2_dot_5_free()),
        ("minimax-m2.7", minimax_m2_dot_7()),
    ]
}
fn minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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

fn minimax_m2_dot_5_free() -> Model {
    Model {
        id: "minimax-m2.5-free".into(),
        name: "MiniMax M2.5 Free".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn minimax_m2_dot_7() -> Model {
    Model {
        id: "minimax-m2.7".into(),
        name: "MiniMax M2.7".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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
