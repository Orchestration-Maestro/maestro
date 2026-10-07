// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("z-ai/glm-4-32b", z_ai_glm_4_32b()),
        ("z-ai/glm-4.5", z_ai_glm_4_dot_5()),
        ("z-ai/glm-4.5-air", z_ai_glm_4_dot_5_air()),
        ("z-ai/glm-4.5-air:free", z_ai_glm_4_dot_5_air_free()),
        ("z-ai/glm-4.5v", z_ai_glm_4_dot_5v()),
        ("z-ai/glm-4.6", z_ai_glm_4_dot_6()),
        ("z-ai/glm-4.6v", z_ai_glm_4_dot_6v()),
        ("z-ai/glm-4.7", z_ai_glm_4_dot_7()),
        ("z-ai/glm-4.7-flash", z_ai_glm_4_dot_7_flash()),
        ("z-ai/glm-5", z_ai_glm_5()),
    ]
}
fn z_ai_glm_4_32b() -> Model {
    Model {
        id: "z-ai/glm-4-32b".into(),
        name: "Z.ai: GLM 4 32B ".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_5() -> Model {
    Model {
        id: "z-ai/glm-4.5".into(),
        name: "Z.ai: GLM 4.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.2,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 98_304.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_5_air() -> Model {
    Model {
        id: "z-ai/glm-4.5-air".into(),
        name: "Z.ai: GLM 4.5 Air".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.13,
            output: 0.85,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 98_304.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_5_air_free() -> Model {
    Model {
        id: "z-ai/glm-4.5-air:free".into(),
        name: "Z.ai: GLM 4.5 Air (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 96_000.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_5v() -> Model {
    Model {
        id: "z-ai/glm-4.5v".into(),
        name: "Z.ai: GLM 4.5V".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 1.799_999_999_999_999_8,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 65_536.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_6() -> Model {
    Model {
        id: "z-ai/glm-4.6".into(),
        name: "Z.ai: GLM 4.6".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.39,
            output: 1.9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 204_800.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_6v() -> Model {
    Model {
        id: "z-ai/glm-4.6v".into(),
        name: "Z.ai: GLM 4.6V".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 0.899_999_999_999_999_9,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 24_000.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_7() -> Model {
    Model {
        id: "z-ai/glm-4.7".into(),
        name: "Z.ai: GLM 4.7".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.38,
            output: 1.74,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_4_dot_7_flash() -> Model {
    Model {
        id: "z-ai/glm-4.7-flash".into(),
        name: "Z.ai: GLM 4.7 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn z_ai_glm_5() -> Model {
    Model {
        id: "z-ai/glm-5".into(),
        name: "Z.ai: GLM 5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 1.9,
            cache_read: 0.119,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
