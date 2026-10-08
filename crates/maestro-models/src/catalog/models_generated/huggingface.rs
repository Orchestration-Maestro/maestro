//! Generated model descriptors for huggingface.

use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_3());
    models.extend(models_9());
    models.extend(models_10());
    models.extend(models_13());
    models.extend(models_18());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_10() -> [(&'static str, Model); 3] {
    [
        (
            "deepseek-ai/DeepSeek-R1-0528",
            deepseek_ai_deepseek_r1_0528(),
        ),
        ("deepseek-ai/DeepSeek-V3.2", deepseek_ai_deepseek_v3_dot_2()),
        ("deepseek-ai/DeepSeek-V4-Pro", deepseek_ai_deepseek_v4_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn deepseek_ai_deepseek_r1_0528() -> Model {
    Model {
        id: "deepseek-ai/DeepSeek-R1-0528".into(),
        name: "DeepSeek-R1-0528".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 5.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 163_840.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_ai_deepseek_v3_dot_2() -> Model {
    Model {
        id: "deepseek-ai/DeepSeek-V3.2".into(),
        name: "DeepSeek-V3.2".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.28,
            output: 0.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_ai_deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek-ai/DeepSeek-V4-Pro".into(),
        name: "DeepSeek V4 Pro".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.74,
            output: 3.48,
            cache_read: 0.145,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 393_216.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 3] {
    [
        ("MiniMaxAI/MiniMax-M2.1", minimaxai_minimax_m2_dot_1()),
        ("MiniMaxAI/MiniMax-M2.5", minimaxai_minimax_m2_dot_5()),
        ("MiniMaxAI/MiniMax-M2.7", minimaxai_minimax_m2_dot_7()),
    ]
}
/// Construct the recorded descriptor for this model.
fn minimaxai_minimax_m2_dot_1() -> Model {
    Model {
        id: "MiniMaxAI/MiniMax-M2.1".into(),
        name: "MiniMax-M2.1".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn minimaxai_minimax_m2_dot_5() -> Model {
    Model {
        id: "MiniMaxAI/MiniMax-M2.5".into(),
        name: "MiniMax-M2.5".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
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
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn minimaxai_minimax_m2_dot_7() -> Model {
    Model {
        id: "MiniMaxAI/MiniMax-M2.7".into(),
        name: "MiniMax-M2.7".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
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
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_13() -> [(&'static str, Model); 5] {
    [
        ("moonshotai/Kimi-K2-Instruct", moonshotai_kimi_k2_instruct()),
        (
            "moonshotai/Kimi-K2-Instruct-0905",
            moonshotai_kimi_k2_instruct_0905(),
        ),
        ("moonshotai/Kimi-K2-Thinking", moonshotai_kimi_k2_thinking()),
        ("moonshotai/Kimi-K2.5", moonshotai_kimi_k2_dot_5()),
        ("moonshotai/Kimi-K2.6", moonshotai_kimi_k2_dot_6()),
    ]
}
/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_instruct() -> Model {
    Model {
        id: "moonshotai/Kimi-K2-Instruct".into(),
        name: "Kimi-K2-Instruct".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_instruct_0905() -> Model {
    Model {
        id: "moonshotai/Kimi-K2-Instruct-0905".into(),
        name: "Kimi-K2-Instruct-0905".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_thinking() -> Model {
    Model {
        id: "moonshotai/Kimi-K2-Thinking".into(),
        name: "Kimi-K2-Thinking".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_dot_5() -> Model {
    Model {
        id: "moonshotai/Kimi-K2.5".into(),
        name: "Kimi-K2.5".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.1,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_dot_6() -> Model {
    Model {
        id: "moonshotai/Kimi-K2.6".into(),
        name: "Kimi-K2.6".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.95,
            output: 4.0,
            cache_read: 0.16,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_3() -> [(&'static str, Model); 6] {
    [
        (
            "Qwen/Qwen3-235B-A22B-Thinking-2507",
            qwen_qwen3_235b_a22b_thinking_2507(),
        ),
        (
            "Qwen/Qwen3-Coder-480B-A35B-Instruct",
            qwen_qwen3_coder_480b_a35b_instruct(),
        ),
        ("Qwen/Qwen3-Coder-Next", qwen_qwen3_coder_next()),
        (
            "Qwen/Qwen3-Next-80B-A3B-Instruct",
            qwen_qwen3_next_80b_a3b_instruct(),
        ),
        (
            "Qwen/Qwen3-Next-80B-A3B-Thinking",
            qwen_qwen3_next_80b_a3b_thinking(),
        ),
        ("Qwen/Qwen3.5-397B-A17B", qwen_qwen3_dot_5_397b_a17b()),
    ]
}
/// Construct the recorded descriptor for this model.
fn qwen_qwen3_235b_a22b_thinking_2507() -> Model {
    Model {
        id: "Qwen/Qwen3-235B-A22B-Thinking-2507".into(),
        name: "Qwen3-235B-A22B-Thinking-2507".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder_480b_a35b_instruct() -> Model {
    Model {
        id: "Qwen/Qwen3-Coder-480B-A35B-Instruct".into(),
        name: "Qwen3-Coder-480B-A35B-Instruct".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 66_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder_next() -> Model {
    Model {
        id: "Qwen/Qwen3-Coder-Next".into(),
        name: "Qwen3-Coder-Next".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_next_80b_a3b_instruct() -> Model {
    Model {
        id: "Qwen/Qwen3-Next-80B-A3B-Instruct".into(),
        name: "Qwen3-Next-80B-A3B-Instruct".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 1.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 66_536.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_next_80b_a3b_thinking() -> Model {
    Model {
        id: "Qwen/Qwen3-Next-80B-A3B-Thinking".into(),
        name: "Qwen3-Next-80B-A3B-Thinking".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_397b_a17b() -> Model {
    Model {
        id: "Qwen/Qwen3.5-397B-A17B".into(),
        name: "Qwen3.5-397B-A17B".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_9() -> [(&'static str, Model); 1] {
    [("XiaomiMiMo/MiMo-V2-Flash", xiaomimimo_mimo_v2_flash())]
}
/// Construct the recorded descriptor for this model.
fn xiaomimimo_mimo_v2_flash() -> Model {
    Model {
        id: "XiaomiMiMo/MiMo-V2-Flash".into(),
        name: "MiMo-V2-Flash".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_18() -> [(&'static str, Model); 4] {
    [
        ("zai-org/GLM-4.7", zai_org_glm_4_dot_7()),
        ("zai-org/GLM-4.7-Flash", zai_org_glm_4_dot_7_flash()),
        ("zai-org/GLM-5", zai_org_glm_5()),
        ("zai-org/GLM-5.1", zai_org_glm_5_dot_1()),
    ]
}
/// Construct the recorded descriptor for this model.
fn zai_org_glm_4_dot_7() -> Model {
    Model {
        id: "zai-org/GLM-4.7".into(),
        name: "GLM-4.7".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.2,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn zai_org_glm_4_dot_7_flash() -> Model {
    Model {
        id: "zai-org/GLM-4.7-Flash".into(),
        name: "GLM-4.7-Flash".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn zai_org_glm_5() -> Model {
    Model {
        id: "zai-org/GLM-5".into(),
        name: "GLM-5".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.2,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn zai_org_glm_5_dot_1() -> Model {
    Model {
        id: "zai-org/GLM-5.1".into(),
        name: "GLM-5.1".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.2,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}
