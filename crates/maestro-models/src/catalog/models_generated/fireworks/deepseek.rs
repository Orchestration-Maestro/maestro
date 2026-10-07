// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 3] {
    [
        (
            "accounts/fireworks/models/deepseek-v3p1",
            accounts_fireworks_models_deepseek_v3p1(),
        ),
        (
            "accounts/fireworks/models/deepseek-v3p2",
            accounts_fireworks_models_deepseek_v3p2(),
        ),
        (
            "accounts/fireworks/models/deepseek-v4-pro",
            accounts_fireworks_models_deepseek_v4_pro(),
        ),
    ]
}
fn accounts_fireworks_models_deepseek_v3p1() -> Model {
    Model {
        id: "accounts/fireworks/models/deepseek-v3p1".into(),
        name: "DeepSeek V3.1".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.56,
            output: 1.68,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 163_840.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_deepseek_v3p2() -> Model {
    Model {
        id: "accounts/fireworks/models/deepseek-v3p2".into(),
        name: "DeepSeek V3.2".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.56,
            output: 1.68,
            cache_read: 0.28,
            cache_write: 0.0,
        },
        context_window: 160_000.0,
        max_tokens: 160_000.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_deepseek_v4_pro() -> Model {
    Model {
        id: "accounts/fireworks/models/deepseek-v4-pro".into(),
        name: "DeepSeek V4 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.74,
            output: 3.48,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 384_000.0,
        headers: None,
        compat: None,
    }
}
