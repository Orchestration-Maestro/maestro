// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_14() -> [(&'static str, Model); 3] {
    [
        (
            "accounts/fireworks/models/minimax-m2p1",
            accounts_fireworks_models_minimax_m2p1(),
        ),
        (
            "accounts/fireworks/models/minimax-m2p5",
            accounts_fireworks_models_minimax_m2p5(),
        ),
        (
            "accounts/fireworks/models/minimax-m2p7",
            accounts_fireworks_models_minimax_m2p7(),
        ),
    ]
}
fn accounts_fireworks_models_minimax_m2p1() -> Model {
    Model {
        id: "accounts/fireworks/models/minimax-m2p1".into(),
        name: "MiniMax-M2.1".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 200_000.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_minimax_m2p5() -> Model {
    Model {
        id: "accounts/fireworks/models/minimax-m2p5".into(),
        name: "MiniMax-M2.5".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 196_608.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_minimax_m2p7() -> Model {
    Model {
        id: "accounts/fireworks/models/minimax-m2p7".into(),
        name: "MiniMax-M2.7".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 196_608.0,
        headers: None,
        compat: None,
    }
}
