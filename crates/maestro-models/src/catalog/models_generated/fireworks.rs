//! Generated model descriptors for fireworks.

use crate::{Model, ModelCost, ModelInput};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_3());
    models.extend(models_8());
    models.extend(models_10());
    models.extend(models_14());
    models.extend(models_17());
    models.extend(models_18());
    models
}

/// Assemble a batch of descriptors in registry order.
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
/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_3() -> [(&'static str, Model); 5] {
    [
        (
            "accounts/fireworks/models/glm-4p5",
            accounts_fireworks_models_glm_4p5(),
        ),
        (
            "accounts/fireworks/models/glm-4p5-air",
            accounts_fireworks_models_glm_4p5_air(),
        ),
        (
            "accounts/fireworks/models/glm-4p7",
            accounts_fireworks_models_glm_4p7(),
        ),
        (
            "accounts/fireworks/models/glm-5",
            accounts_fireworks_models_glm_5(),
        ),
        (
            "accounts/fireworks/models/glm-5p1",
            accounts_fireworks_models_glm_5p1(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
fn accounts_fireworks_models_glm_4p5() -> Model {
    Model {
        id: "accounts/fireworks/models/glm-4p5".into(),
        name: "GLM 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.55,
            output: 2.19,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
fn accounts_fireworks_models_glm_4p5_air() -> Model {
    Model {
        id: "accounts/fireworks/models/glm-4p5-air".into(),
        name: "GLM 4.5 Air".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.88,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
fn accounts_fireworks_models_glm_4p7() -> Model {
    Model {
        id: "accounts/fireworks/models/glm-4p7".into(),
        name: "GLM 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.2,
            cache_read: 0.3,
            cache_write: 0.0,
        },
        context_window: 198_000.0,
        max_tokens: 198_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
fn accounts_fireworks_models_glm_5() -> Model {
    Model {
        id: "accounts/fireworks/models/glm-5".into(),
        name: "GLM 5".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.2,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
fn accounts_fireworks_models_glm_5p1() -> Model {
    Model {
        id: "accounts/fireworks/models/glm-5p1".into(),
        name: "GLM 5.1".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 202_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_8() -> [(&'static str, Model); 2] {
    [
        (
            "accounts/fireworks/models/gpt-oss-120b",
            accounts_fireworks_models_gpt_oss_120b(),
        ),
        (
            "accounts/fireworks/models/gpt-oss-20b",
            accounts_fireworks_models_gpt_oss_20b(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
fn accounts_fireworks_models_gpt_oss_120b() -> Model {
    Model {
        id: "accounts/fireworks/models/gpt-oss-120b".into(),
        name: "GPT OSS 120B".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
fn accounts_fireworks_models_gpt_oss_20b() -> Model {
    Model {
        id: "accounts/fireworks/models/gpt-oss-20b".into(),
        name: "GPT OSS 20B".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.05,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_10() -> [(&'static str, Model); 4] {
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
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_18() -> [(&'static str, Model); 1] {
    [(
        "accounts/fireworks/routers/kimi-k2p5-turbo",
        accounts_fireworks_routers_kimi_k2p5_turbo(),
    )]
}
/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
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
/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_17() -> [(&'static str, Model); 1] {
    [(
        "accounts/fireworks/models/qwen3p6-plus",
        accounts_fireworks_models_qwen3p6_plus(),
    )]
}
/// Assemble a batch of descriptors in registry order.
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}
