// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 5] {
    [
        ("mimo-v2-flash", mimo_v2_flash()),
        ("mimo-v2-omni", mimo_v2_omni()),
        ("mimo-v2-pro", mimo_v2_pro()),
        ("mimo-v2.5", mimo_v2_dot_5()),
        ("mimo-v2.5-pro", mimo_v2_dot_5_pro()),
    ]
}
fn mimo_v2_flash() -> Model {
    Model {
        id: "mimo-v2-flash".into(),
        name: "MiMo-V2-Flash".into(),
        api: "anthropic-messages".into(),
        provider: "xiaomi-token-plan-sgp".into(),
        base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn mimo_v2_omni() -> Model {
    Model {
        id: "mimo-v2-omni".into(),
        name: "MiMo-V2-Omni".into(),
        api: "anthropic-messages".into(),
        provider: "xiaomi-token-plan-sgp".into(),
        base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn mimo_v2_pro() -> Model {
    Model {
        id: "mimo-v2-pro".into(),
        name: "MiMo-V2-Pro".into(),
        api: "anthropic-messages".into(),
        provider: "xiaomi-token-plan-sgp".into(),
        base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn mimo_v2_dot_5() -> Model {
    Model {
        id: "mimo-v2.5".into(),
        name: "MiMo-V2.5".into(),
        api: "anthropic-messages".into(),
        provider: "xiaomi-token-plan-sgp".into(),
        base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn mimo_v2_dot_5_pro() -> Model {
    Model {
        id: "mimo-v2.5-pro".into(),
        name: "MiMo-V2.5-Pro".into(),
        api: "anthropic-messages".into(),
        provider: "xiaomi-token-plan-sgp".into(),
        base_url: "https://token-plan-sgp.xiaomimimo.com/anthropic".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
