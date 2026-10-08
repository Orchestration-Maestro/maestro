//! Generated model descriptors for groq.

use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_1());
    models.extend(models_2());
    models.extend(models_4());
    models.extend(models_8());
    models.extend(models_10());
    models.extend(models_11());
    models.extend(models_13());
    models.extend(models_16());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [(
        "deepseek-r1-distill-llama-70b",
        deepseek_r1_distill_llama_70b(),
    )]
}
/// Construct the recorded descriptor for this model.
fn deepseek_r1_distill_llama_70b() -> Model {
    Model {
        id: "deepseek-r1-distill-llama-70b".into(),
        name: "DeepSeek R1 Distill Llama 70B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.75,
            output: 0.99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_1() -> [(&'static str, Model); 1] {
    [("gemma2-9b-it", gemma2_9b_it())]
}
/// Construct the recorded descriptor for this model.
fn gemma2_9b_it() -> Model {
    Model {
        id: "gemma2-9b-it".into(),
        name: "Gemma 2 9B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_2() -> [(&'static str, Model); 2] {
    [
        ("groq/compound", groq_compound()),
        ("groq/compound-mini", groq_compound_mini()),
    ]
}
/// Construct the recorded descriptor for this model.
fn groq_compound() -> Model {
    Model {
        id: "groq/compound".into(),
        name: "Compound".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn groq_compound_mini() -> Model {
    Model {
        id: "groq/compound-mini".into(),
        name: "Compound Mini".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_4() -> [(&'static str, Model); 4] {
    [
        ("llama-3.1-8b-instant", llama_3_dot_1_8b_instant()),
        ("llama-3.3-70b-versatile", llama_3_dot_3_70b_versatile()),
        ("llama3-70b-8192", llama3_70b_8192()),
        ("llama3-8b-8192", llama3_8b_8192()),
    ]
}
/// Construct the recorded descriptor for this model.
fn llama_3_dot_1_8b_instant() -> Model {
    Model {
        id: "llama-3.1-8b-instant".into(),
        name: "Llama 3.1 8B Instant".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.05,
            output: 0.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn llama_3_dot_3_70b_versatile() -> Model {
    Model {
        id: "llama-3.3-70b-versatile".into(),
        name: "Llama 3.3 70B Versatile".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.59,
            output: 0.79,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn llama3_70b_8192() -> Model {
    Model {
        id: "llama3-70b-8192".into(),
        name: "Llama 3 70B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.59,
            output: 0.79,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn llama3_8b_8192() -> Model {
    Model {
        id: "llama3-8b-8192".into(),
        name: "Llama 3 8B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.05,
            output: 0.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_8() -> [(&'static str, Model); 2] {
    [
        (
            "meta-llama/llama-4-maverick-17b-128e-instruct",
            meta_llama_llama_4_maverick_17b_128e_instruct(),
        ),
        (
            "meta-llama/llama-4-scout-17b-16e-instruct",
            meta_llama_llama_4_scout_17b_16e_instruct(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn meta_llama_llama_4_maverick_17b_128e_instruct() -> Model {
    Model {
        id: "meta-llama/llama-4-maverick-17b-128e-instruct".into(),
        name: "Llama 4 Maverick 17B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_llama_4_scout_17b_16e_instruct() -> Model {
    Model {
        id: "meta-llama/llama-4-scout-17b-16e-instruct".into(),
        name: "Llama 4 Scout 17B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.11,
            output: 0.34,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_10() -> [(&'static str, Model); 1] {
    [("mistral-saba-24b", mistral_saba_24b())]
}
/// Construct the recorded descriptor for this model.
fn mistral_saba_24b() -> Model {
    Model {
        id: "mistral-saba-24b".into(),
        name: "Mistral Saba 24B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.79,
            output: 0.79,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_11() -> [(&'static str, Model); 2] {
    [
        ("moonshotai/kimi-k2-instruct", moonshotai_kimi_k2_instruct()),
        (
            "moonshotai/kimi-k2-instruct-0905",
            moonshotai_kimi_k2_instruct_0905(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_instruct() -> Model {
    Model {
        id: "moonshotai/kimi-k2-instruct".into(),
        name: "Kimi K2 Instruct".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
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
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_instruct_0905() -> Model {
    Model {
        id: "moonshotai/kimi-k2-instruct-0905".into(),
        name: "Kimi K2 Instruct 0905".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
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
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_13() -> [(&'static str, Model); 3] {
    [
        ("openai/gpt-oss-120b", openai_gpt_oss_120b()),
        ("openai/gpt-oss-20b", openai_gpt_oss_20b()),
        (
            "openai/gpt-oss-safeguard-20b",
            openai_gpt_oss_safeguard_20b(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_120b() -> Model {
    Model {
        id: "openai/gpt-oss-120b".into(),
        name: "GPT OSS 120B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
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
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_20b() -> Model {
    Model {
        id: "openai/gpt-oss-20b".into(),
        name: "GPT OSS 20B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_safeguard_20b() -> Model {
    Model {
        id: "openai/gpt-oss-safeguard-20b".into(),
        name: "Safety GPT OSS 20B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.037,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_16() -> [(&'static str, Model); 2] {
    [
        ("qwen-qwq-32b", qwen_qwq_32b()),
        ("qwen/qwen3-32b", qwen_qwen3_32b()),
    ]
}
/// Construct the recorded descriptor for this model.
fn qwen_qwq_32b() -> Model {
    Model {
        id: "qwen-qwq-32b".into(),
        name: "Qwen QwQ 32B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.29,
            output: 0.39,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_32b() -> Model {
    Model {
        id: "qwen/qwen3-32b".into(),
        name: "Qwen3 32B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, None),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("default".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.29,
            output: 0.59,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 40_960.0,
        headers: None,
        compat: None,
    }
}
