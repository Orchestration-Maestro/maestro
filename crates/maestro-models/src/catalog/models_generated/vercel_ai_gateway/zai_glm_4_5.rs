// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("zai/glm-4.5", zai_glm_4_dot_5()),
        ("zai/glm-4.5-air", zai_glm_4_dot_5_air()),
        ("zai/glm-4.5v", zai_glm_4_dot_5v()),
        ("zai/glm-4.6", zai_glm_4_dot_6()),
        ("zai/glm-4.6v", zai_glm_4_dot_6v()),
        ("zai/glm-4.6v-flash", zai_glm_4_dot_6v_flash()),
        ("zai/glm-4.7", zai_glm_4_dot_7()),
        ("zai/glm-4.7-flash", zai_glm_4_dot_7_flash()),
        ("zai/glm-4.7-flashx", zai_glm_4_dot_7_flashx()),
        ("zai/glm-5", zai_glm_5()),
    ]
}
fn zai_glm_4_dot_5() -> Model {
    Model {
        id: "zai/glm-4.5".into(),
        name: "GLM-4.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.2,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 96_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_5_air() -> Model {
    Model {
        id: "zai/glm-4.5-air".into(),
        name: "GLM 4.5 Air".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.1,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 96_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_5v() -> Model {
    Model {
        id: "zai/glm-4.5v".into(),
        name: "GLM 4.5V".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 1.799_999_999_999_999_8,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 66_000.0,
        max_tokens: 16_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_6() -> Model {
    Model {
        id: "zai/glm-4.6".into(),
        name: "GLM 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.2,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 96_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_6v() -> Model {
    Model {
        id: "zai/glm-4.6v".into(),
        name: "GLM-4.6V".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 0.899_999_999_999_999_9,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 24_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_6v_flash() -> Model {
    Model {
        id: "zai/glm-4.6v-flash".into(),
        name: "GLM-4.6V-Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 24_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_7() -> Model {
    Model {
        id: "zai/glm-4.7".into(),
        name: "GLM 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.25,
            output: 2.75,
            cache_read: 2.25,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_7_flash() -> Model {
    Model {
        id: "zai/glm-4.7-flash".into(),
        name: "GLM 4.7 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_4_dot_7_flashx() -> Model {
    Model {
        id: "zai/glm-4.7-flashx".into(),
        name: "GLM 4.7 FlashX".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn zai_glm_5() -> Model {
    Model {
        id: "zai/glm-5".into(),
        name: "GLM 5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.199_999_999_999_999_7,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 202_800.0,
        max_tokens: 131_100.0,
        headers: None,
        compat: None,
    }
}
