// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
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
