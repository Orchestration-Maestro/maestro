// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 9] {
    [
        (
            "accounts/fireworks/models/kimi-k2-instruct",
            accounts_fireworks_models_kimi_k2_instruct(),
        ),
        (
            "accounts/fireworks/models/kimi-k2-thinking",
            accounts_fireworks_models_kimi_k2_thinking(),
        ),
        (
            "accounts/fireworks/models/kimi-k2p5",
            accounts_fireworks_models_kimi_k2p5(),
        ),
        (
            "accounts/fireworks/models/kimi-k2p6",
            accounts_fireworks_models_kimi_k2p6(),
        ),
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
        (
            "accounts/fireworks/models/qwen3p6-plus",
            accounts_fireworks_models_qwen3p6_plus(),
        ),
        (
            "accounts/fireworks/routers/kimi-k2p5-turbo",
            accounts_fireworks_routers_kimi_k2p5_turbo(),
        ),
    ]
}
fn accounts_fireworks_models_kimi_k2_instruct() -> Model {
    Model {
        id: "accounts/fireworks/models/kimi-k2-instruct".into(),
        name: "Kimi K2 Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_kimi_k2_thinking() -> Model {
    Model {
        id: "accounts/fireworks/models/kimi-k2-thinking".into(),
        name: "Kimi K2 Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.3,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_kimi_k2p5() -> Model {
    Model {
        id: "accounts/fireworks/models/kimi-k2p5".into(),
        name: "Kimi K2.5".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_kimi_k2p6() -> Model {
    Model {
        id: "accounts/fireworks/models/kimi-k2p6".into(),
        name: "Kimi K2.6".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
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

fn accounts_fireworks_models_qwen3p6_plus() -> Model {
    Model {
        id: "accounts/fireworks/models/qwen3p6-plus".into(),
        name: "Qwen 3.6 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_routers_kimi_k2p5_turbo() -> Model {
    Model {
        id: "accounts/fireworks/routers/kimi-k2p5-turbo".into(),
        name: "Kimi K2.5 Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}
