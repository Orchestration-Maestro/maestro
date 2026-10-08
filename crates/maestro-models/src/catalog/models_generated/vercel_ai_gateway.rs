//! Generated model descriptors for vercel-ai-gateway.

use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_10());
    models.extend(models_18());
    models.extend(models_28());
    models.extend(models_30());
    models.extend(models_32());
    models.extend(models_33());
    models.extend(models_34());
    models.extend(models_42());
    models.extend(models_52());
    models.extend(models_53());
    models.extend(models_55());
    models.extend(models_56());
    models.extend(models_57());
    models.extend(models_64());
    models.extend(models_71());
    models.extend(models_81());
    models.extend(models_87());
    models.extend(models_89());
    models.extend(models_99());
    models.extend(models_109());
    models.extend(models_119());
    models.extend(models_126());
    models.extend(models_128());
    models.extend(models_138());
    models.extend(models_145());
    models.extend(models_149());
    models.extend(models_159());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 10] {
    [
        ("alibaba/qwen-3-14b", alibaba_qwen_3_14b()),
        ("alibaba/qwen-3-235b", alibaba_qwen_3_235b()),
        ("alibaba/qwen-3-30b", alibaba_qwen_3_30b()),
        ("alibaba/qwen-3-32b", alibaba_qwen_3_32b()),
        (
            "alibaba/qwen-3.6-max-preview",
            alibaba_qwen_3_dot_6_max_preview(),
        ),
        (
            "alibaba/qwen3-235b-a22b-thinking",
            alibaba_qwen3_235b_a22b_thinking(),
        ),
        ("alibaba/qwen3-coder", alibaba_qwen3_coder()),
        ("alibaba/qwen3-coder-30b-a3b", alibaba_qwen3_coder_30b_a3b()),
        ("alibaba/qwen3-coder-next", alibaba_qwen3_coder_next()),
        ("alibaba/qwen3-coder-plus", alibaba_qwen3_coder_plus()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_10() -> [(&'static str, Model); 8] {
    [
        ("alibaba/qwen3-max", alibaba_qwen3_max()),
        ("alibaba/qwen3-max-preview", alibaba_qwen3_max_preview()),
        ("alibaba/qwen3-max-thinking", alibaba_qwen3_max_thinking()),
        ("alibaba/qwen3-vl-thinking", alibaba_qwen3_vl_thinking()),
        ("alibaba/qwen3.5-flash", alibaba_qwen3_dot_5_flash()),
        ("alibaba/qwen3.5-plus", alibaba_qwen3_dot_5_plus()),
        ("alibaba/qwen3.6-27b", alibaba_qwen3_dot_6_27b()),
        ("alibaba/qwen3.6-plus", alibaba_qwen3_dot_6_plus()),
    ]
}
/// Construct the recorded descriptor for this model.
fn alibaba_qwen_3_14b() -> Model {
    Model {
        id: "alibaba/qwen-3-14b".into(),
        name: "Qwen3-14B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.12,
            output: 0.24,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen_3_235b() -> Model {
    Model {
        id: "alibaba/qwen-3-235b".into(),
        name: "Qwen3 235B A22b Instruct 2507".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 1.2,
            cache_read: 0.6,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen_3_30b() -> Model {
    Model {
        id: "alibaba/qwen-3-30b".into(),
        name: "Qwen3-30B-A3B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.29,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen_3_32b() -> Model {
    Model {
        id: "alibaba/qwen-3-32b".into(),
        name: "Qwen 3 32B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.16,
            output: 0.64,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen_3_dot_6_max_preview() -> Model {
    Model {
        id: "alibaba/qwen-3.6-max-preview".into(),
        name: "Qwen 3.6 Max Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.3,
            output: 7.8,
            cache_read: 0.26,
            cache_write: 1.625,
        },
        context_window: 240_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_235b_a22b_thinking() -> Model {
    Model {
        id: "alibaba/qwen3-235b-a22b-thinking".into(),
        name: "Qwen3 VL 235B A22B Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 4.0,
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
fn alibaba_qwen3_coder() -> Model {
    Model {
        id: "alibaba/qwen3-coder".into(),
        name: "Qwen3 Coder 480B A35B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.5,
            output: 7.5,
            cache_read: 0.3,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_coder_30b_a3b() -> Model {
    Model {
        id: "alibaba/qwen3-coder-30b-a3b".into(),
        name: "Qwen 3 Coder 30B A3B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_coder_next() -> Model {
    Model {
        id: "alibaba/qwen3-coder-next".into(),
        name: "Qwen3 Coder Next".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_coder_plus() -> Model {
    Model {
        id: "alibaba/qwen3-coder-plus".into(),
        name: "Qwen3 Coder Plus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_max() -> Model {
    Model {
        id: "alibaba/qwen3-max".into(),
        name: "Qwen3 Max".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 6.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_max_preview() -> Model {
    Model {
        id: "alibaba/qwen3-max-preview".into(),
        name: "Qwen3 Max Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 6.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_max_thinking() -> Model {
    Model {
        id: "alibaba/qwen3-max-thinking".into(),
        name: "Qwen 3 Max Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 6.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_vl_thinking() -> Model {
    Model {
        id: "alibaba/qwen3-vl-thinking".into(),
        name: "Qwen3 VL 235B A22B Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 4.0,
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
fn alibaba_qwen3_dot_5_flash() -> Model {
    Model {
        id: "alibaba/qwen3.5-flash".into(),
        name: "Qwen 3.5 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.001,
            cache_write: 0.125,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_dot_5_plus() -> Model {
    Model {
        id: "alibaba/qwen3.5-plus".into(),
        name: "Qwen 3.5 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.4,
            cache_read: 0.04,
            cache_write: 0.5,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_dot_6_27b() -> Model {
    Model {
        id: "alibaba/qwen3.6-27b".into(),
        name: "Qwen 3.6 27B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.599_999_999_999_999_6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn alibaba_qwen3_dot_6_plus() -> Model {
    Model {
        id: "alibaba/qwen3.6-plus".into(),
        name: "Qwen 3.6 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 0.625,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_18() -> [(&'static str, Model); 10] {
    [
        ("anthropic/claude-3-haiku", anthropic_claude_3_haiku()),
        (
            "anthropic/claude-3.5-haiku",
            anthropic_claude_3_dot_5_haiku(),
        ),
        (
            "anthropic/claude-3.7-sonnet",
            anthropic_claude_3_dot_7_sonnet(),
        ),
        (
            "anthropic/claude-haiku-4.5",
            anthropic_claude_haiku_4_dot_5(),
        ),
        ("anthropic/claude-opus-4", anthropic_claude_opus_4()),
        ("anthropic/claude-opus-4.1", anthropic_claude_opus_4_dot_1()),
        ("anthropic/claude-opus-4.5", anthropic_claude_opus_4_dot_5()),
        ("anthropic/claude-opus-4.6", anthropic_claude_opus_4_dot_6()),
        ("anthropic/claude-opus-4.7", anthropic_claude_opus_4_dot_7()),
        ("anthropic/claude-sonnet-4", anthropic_claude_sonnet_4()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_28() -> [(&'static str, Model); 2] {
    [
        (
            "anthropic/claude-sonnet-4.5",
            anthropic_claude_sonnet_4_dot_5(),
        ),
        (
            "anthropic/claude-sonnet-4.6",
            anthropic_claude_sonnet_4_dot_6(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn anthropic_claude_3_haiku() -> Model {
    Model {
        id: "anthropic/claude-3-haiku".into(),
        name: "Claude 3 Haiku".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.25,
            cache_read: 0.03,
            cache_write: 0.3,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_3_dot_5_haiku() -> Model {
    Model {
        id: "anthropic/claude-3.5-haiku".into(),
        name: "Claude 3.5 Haiku".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.799_999_999_999_999_9,
            output: 4.0,
            cache_read: 0.08,
            cache_write: 1.0,
        },
        context_window: 200_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_3_dot_7_sonnet() -> Model {
    Model {
        id: "anthropic/claude-3.7-sonnet".into(),
        name: "Claude 3.7 Sonnet".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 200_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_haiku_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-haiku-4.5".into(),
        name: "Claude Haiku 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 1.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_opus_4() -> Model {
    Model {
        id: "anthropic/claude-opus-4".into(),
        name: "Claude Opus 4".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_opus_4_dot_1() -> Model {
    Model {
        id: "anthropic/claude-opus-4.1".into(),
        name: "Claude Opus 4.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 75.0,
            cache_read: 1.5,
            cache_write: 18.75,
        },
        context_window: 200_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_opus_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-opus-4.5".into(),
        name: "Claude Opus 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_opus_4_dot_6() -> Model {
    Model {
        id: "anthropic/claude-opus-4.6".into(),
        name: "Claude Opus 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_opus_4_dot_7() -> Model {
    Model {
        id: "anthropic/claude-opus-4.7".into(),
        name: "Claude Opus 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 0.5,
            cache_write: 6.25,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_sonnet_4() -> Model {
    Model {
        id: "anthropic/claude-sonnet-4".into(),
        name: "Claude Sonnet 4".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_sonnet_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-sonnet-4.5".into(),
        name: "Claude Sonnet 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_sonnet_4_dot_6() -> Model {
    Model {
        id: "anthropic/claude-sonnet-4.6".into(),
        name: "Claude Sonnet 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.3,
            cache_write: 3.75,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_30() -> [(&'static str, Model); 2] {
    [
        (
            "arcee-ai/trinity-large-preview",
            arcee_ai_trinity_large_preview(),
        ),
        (
            "arcee-ai/trinity-large-thinking",
            arcee_ai_trinity_large_thinking(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn arcee_ai_trinity_large_preview() -> Model {
    Model {
        id: "arcee-ai/trinity-large-preview".into(),
        name: "Trinity Large Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 1.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn arcee_ai_trinity_large_thinking() -> Model {
    Model {
        id: "arcee-ai/trinity-large-thinking".into(),
        name: "Trinity Large Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.899_999_999_999_999_9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_100.0,
        max_tokens: 80_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_32() -> [(&'static str, Model); 1] {
    [("bytedance/seed-1.6", bytedance_seed_1_dot_6())]
}
/// Construct the recorded descriptor for this model.
fn bytedance_seed_1_dot_6() -> Model {
    Model {
        id: "bytedance/seed-1.6".into(),
        name: "Seed 1.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_33() -> [(&'static str, Model); 1] {
    [("cohere/command-a", cohere_command_a())]
}
/// Construct the recorded descriptor for this model.
fn cohere_command_a() -> Model {
    Model {
        id: "cohere/command-a".into(),
        name: "Command A".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_34() -> [(&'static str, Model); 8] {
    [
        ("deepseek/deepseek-r1", deepseek_deepseek_r1()),
        ("deepseek/deepseek-v3", deepseek_deepseek_v3()),
        ("deepseek/deepseek-v3.1", deepseek_deepseek_v3_dot_1()),
        (
            "deepseek/deepseek-v3.1-terminus",
            deepseek_deepseek_v3_dot_1_terminus(),
        ),
        ("deepseek/deepseek-v3.2", deepseek_deepseek_v3_dot_2()),
        (
            "deepseek/deepseek-v3.2-thinking",
            deepseek_deepseek_v3_dot_2_thinking(),
        ),
        ("deepseek/deepseek-v4-flash", deepseek_deepseek_v4_flash()),
        ("deepseek/deepseek-v4-pro", deepseek_deepseek_v4_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_r1() -> Model {
    Model {
        id: "deepseek/deepseek-r1".into(),
        name: "DeepSeek-R1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.35,
            output: 5.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3() -> Model {
    Model {
        id: "deepseek/deepseek-v3".into(),
        name: "DeepSeek V3 0324".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.77,
            output: 0.77,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3_dot_1() -> Model {
    Model {
        id: "deepseek/deepseek-v3.1".into(),
        name: "DeepSeek-V3.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.56,
            output: 1.68,
            cache_read: 0.28,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3_dot_1_terminus() -> Model {
    Model {
        id: "deepseek/deepseek-v3.1-terminus".into(),
        name: "DeepSeek V3.1 Terminus".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.27,
            output: 1.0,
            cache_read: 0.135,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3_dot_2() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2".into(),
        name: "DeepSeek V3.2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.28,
            output: 0.42,
            cache_read: 0.028,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3_dot_2_thinking() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2-thinking".into(),
        name: "DeepSeek V3.2 Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.62,
            output: 1.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v4_flash() -> Model {
    Model {
        id: "deepseek/deepseek-v4-flash".into(),
        name: "DeepSeek V4 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 384_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek/deepseek-v4-pro".into(),
        name: "DeepSeek V4 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.435,
            output: 0.87,
            cache_read: 0.0036,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 384_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_42() -> [(&'static str, Model); 10] {
    [
        ("google/gemini-2.0-flash", google_gemini_2_dot_0_flash()),
        (
            "google/gemini-2.0-flash-lite",
            google_gemini_2_dot_0_flash_lite(),
        ),
        ("google/gemini-2.5-flash", google_gemini_2_dot_5_flash()),
        (
            "google/gemini-2.5-flash-lite",
            google_gemini_2_dot_5_flash_lite(),
        ),
        ("google/gemini-2.5-pro", google_gemini_2_dot_5_pro()),
        ("google/gemini-3-flash", google_gemini_3_flash()),
        ("google/gemini-3-pro-preview", google_gemini_3_pro_preview()),
        (
            "google/gemini-3.1-flash-lite-preview",
            google_gemini_3_dot_1_flash_lite_preview(),
        ),
        (
            "google/gemini-3.1-pro-preview",
            google_gemini_3_dot_1_pro_preview(),
        ),
        ("google/gemma-4-26b-a4b-it", google_gemma_4_26b_a4b_it()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_52() -> [(&'static str, Model); 1] {
    [("google/gemma-4-31b-it", google_gemma_4_31b_it())]
}
/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_0_flash() -> Model {
    Model {
        id: "google/gemini-2.0-flash".into(),
        name: "Gemini 2.0 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_0_flash_lite() -> Model {
    Model {
        id: "google/gemini-2.0-flash-lite".into(),
        name: "Gemini 2.0 Flash Lite".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_flash() -> Model {
    Model {
        id: "google/gemini-2.5-flash".into(),
        name: "Gemini 2.5 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_flash_lite() -> Model {
    Model {
        id: "google/gemini-2.5-flash-lite".into(),
        name: "Gemini 2.5 Flash Lite".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_pro() -> Model {
    Model {
        id: "google/gemini-2.5-pro".into(),
        name: "Gemini 2.5 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_flash() -> Model {
    Model {
        id: "google/gemini-3-flash".into(),
        name: "Gemini 3 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_pro_preview() -> Model {
    Model {
        id: "google/gemini-3-pro-preview".into(),
        name: "Gemini 3 Pro Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_dot_1_flash_lite_preview() -> Model {
    Model {
        id: "google/gemini-3.1-flash-lite-preview".into(),
        name: "Gemini 3.1 Flash Lite Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_dot_1_pro_preview() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview".into(),
        name: "Gemini 3.1 Pro Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemma_4_26b_a4b_it() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it".into(),
        name: "Gemma 4 26B A4B IT".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemma_4_31b_it() -> Model {
    Model {
        id: "google/gemma-4-31b-it".into(),
        name: "Gemma 4 31B IT".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.14,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_53() -> [(&'static str, Model); 2] {
    [
        ("inception/mercury-2", inception_mercury_2()),
        (
            "inception/mercury-coder-small",
            inception_mercury_coder_small(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn inception_mercury_2() -> Model {
    Model {
        id: "inception/mercury-2".into(),
        name: "Mercury 2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.75,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn inception_mercury_coder_small() -> Model {
    Model {
        id: "inception/mercury-coder-small".into(),
        name: "Mercury Coder Small Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 1.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_55() -> [(&'static str, Model); 1] {
    [("kwaipilot/kat-coder-pro-v2", kwaipilot_kat_coder_pro_v2())]
}
/// Construct the recorded descriptor for this model.
fn kwaipilot_kat_coder_pro_v2() -> Model {
    Model {
        id: "kwaipilot/kat-coder-pro-v2".into(),
        name: "Kat Coder Pro V2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_56() -> [(&'static str, Model); 1] {
    [("meituan/longcat-flash-chat", meituan_longcat_flash_chat())]
}
/// Construct the recorded descriptor for this model.
fn meituan_longcat_flash_chat() -> Model {
    Model {
        id: "meituan/longcat-flash-chat".into(),
        name: "LongCat Flash Chat".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_57() -> [(&'static str, Model); 7] {
    [
        ("meta/llama-3.1-70b", meta_llama_3_dot_1_70b()),
        ("meta/llama-3.1-8b", meta_llama_3_dot_1_8b()),
        ("meta/llama-3.2-11b", meta_llama_3_dot_2_11b()),
        ("meta/llama-3.2-90b", meta_llama_3_dot_2_90b()),
        ("meta/llama-3.3-70b", meta_llama_3_dot_3_70b()),
        ("meta/llama-4-maverick", meta_llama_4_maverick()),
        ("meta/llama-4-scout", meta_llama_4_scout()),
    ]
}
/// Construct the recorded descriptor for this model.
fn meta_llama_3_dot_1_70b() -> Model {
    Model {
        id: "meta/llama-3.1-70b".into(),
        name: "Llama 3.1 70B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_3_dot_1_8b() -> Model {
    Model {
        id: "meta/llama-3.1-8b".into(),
        name: "Llama 3.1 8B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.22,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_3_dot_2_11b() -> Model {
    Model {
        id: "meta/llama-3.2-11b".into(),
        name: "Llama 3.2 11B Vision Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.16,
            output: 0.16,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_3_dot_2_90b() -> Model {
    Model {
        id: "meta/llama-3.2-90b".into(),
        name: "Llama 3.2 90B Vision Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_3_dot_3_70b() -> Model {
    Model {
        id: "meta/llama-3.3-70b".into(),
        name: "Llama 3.3 70B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_4_maverick() -> Model {
    Model {
        id: "meta/llama-4-maverick".into(),
        name: "Llama 4 Maverick 17B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.24,
            output: 0.970_000_000_000_000_1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_4_scout() -> Model {
    Model {
        id: "meta/llama-4-scout".into(),
        name: "Llama 4 Scout 17B Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.169_999_999_999_999_98,
            output: 0.66,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_64() -> [(&'static str, Model); 7] {
    [
        ("minimax/minimax-m2", minimax_minimax_m2()),
        ("minimax/minimax-m2.1", minimax_minimax_m2_dot_1()),
        (
            "minimax/minimax-m2.1-lightning",
            minimax_minimax_m2_dot_1_lightning(),
        ),
        ("minimax/minimax-m2.5", minimax_minimax_m2_dot_5()),
        (
            "minimax/minimax-m2.5-highspeed",
            minimax_minimax_m2_dot_5_highspeed(),
        ),
        ("minimax/minimax-m2.7", minimax_minimax_m2_dot_7()),
        (
            "minimax/minimax-m2.7-highspeed",
            minimax_minimax_m2_dot_7_highspeed(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2() -> Model {
    Model {
        id: "minimax/minimax-m2".into(),
        name: "MiniMax M2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 205_000.0,
        max_tokens: 205_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_1() -> Model {
    Model {
        id: "minimax/minimax-m2.1".into(),
        name: "MiniMax M2.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_1_lightning() -> Model {
    Model {
        id: "minimax/minimax-m2.1-lightning".into(),
        name: "MiniMax M2.1 Lightning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 2.4,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax/minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_5_highspeed() -> Model {
    Model {
        id: "minimax/minimax-m2.5-highspeed".into(),
        name: "MiniMax M2.5 High Speed".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.03,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_7() -> Model {
    Model {
        id: "minimax/minimax-m2.7".into(),
        name: "Minimax M2.7".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_7_highspeed() -> Model {
    Model {
        id: "minimax/minimax-m2.7-highspeed".into(),
        name: "MiniMax M2.7 High Speed".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.06,
            cache_write: 0.375,
        },
        context_window: 204_800.0,
        max_tokens: 131_100.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_71() -> [(&'static str, Model); 10] {
    [
        ("mistral/codestral", mistral_codestral()),
        ("mistral/devstral-2", mistral_devstral_2()),
        ("mistral/devstral-small", mistral_devstral_small()),
        ("mistral/devstral-small-2", mistral_devstral_small_2()),
        ("mistral/ministral-3b", mistral_ministral_3b()),
        ("mistral/ministral-8b", mistral_ministral_8b()),
        ("mistral/mistral-medium", mistral_mistral_medium()),
        ("mistral/mistral-small", mistral_mistral_small()),
        ("mistral/pixtral-12b", mistral_pixtral_12b()),
        ("mistral/pixtral-large", mistral_pixtral_large()),
    ]
}
/// Construct the recorded descriptor for this model.
fn mistral_codestral() -> Model {
    Model {
        id: "mistral/codestral".into(),
        name: "Mistral Codestral".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.899_999_999_999_999_9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_devstral_2() -> Model {
    Model {
        id: "mistral/devstral-2".into(),
        name: "Devstral 2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_devstral_small() -> Model {
    Model {
        id: "mistral/devstral-small".into(),
        name: "Devstral Small 1.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_devstral_small_2() -> Model {
    Model {
        id: "mistral/devstral-small-2".into(),
        name: "Devstral Small 2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_ministral_3b() -> Model {
    Model {
        id: "mistral/ministral-3b".into(),
        name: "Ministral 3B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_ministral_8b() -> Model {
    Model {
        id: "mistral/ministral-8b".into(),
        name: "Ministral 8B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_mistral_medium() -> Model {
    Model {
        id: "mistral/mistral-medium".into(),
        name: "Mistral Medium 3.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_mistral_small() -> Model {
    Model {
        id: "mistral/mistral-small".into(),
        name: "Mistral Small".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_pixtral_12b() -> Model {
    Model {
        id: "mistral/pixtral-12b".into(),
        name: "Pixtral 12B 2409".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_pixtral_large() -> Model {
    Model {
        id: "mistral/pixtral-large".into(),
        name: "Pixtral Large".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_81() -> [(&'static str, Model); 6] {
    [
        ("moonshotai/kimi-k2", moonshotai_kimi_k2()),
        ("moonshotai/kimi-k2-thinking", moonshotai_kimi_k2_thinking()),
        (
            "moonshotai/kimi-k2-thinking-turbo",
            moonshotai_kimi_k2_thinking_turbo(),
        ),
        ("moonshotai/kimi-k2-turbo", moonshotai_kimi_k2_turbo()),
        ("moonshotai/kimi-k2.5", moonshotai_kimi_k2_dot_5()),
        ("moonshotai/kimi-k2.6", moonshotai_kimi_k2_dot_6()),
    ]
}
/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2() -> Model {
    Model {
        id: "moonshotai/kimi-k2".into(),
        name: "Kimi K2 Instruct".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.570_000_000_000_000_1,
            output: 2.3,
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
fn moonshotai_kimi_k2_thinking() -> Model {
    Model {
        id: "moonshotai/kimi-k2-thinking".into(),
        name: "Kimi K2 Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_114.0,
        max_tokens: 262_114.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_thinking_turbo() -> Model {
    Model {
        id: "moonshotai/kimi-k2-thinking-turbo".into(),
        name: "Kimi K2 Thinking Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.15,
            output: 8.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 262_114.0,
        max_tokens: 262_114.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_turbo() -> Model {
    Model {
        id: "moonshotai/kimi-k2-turbo".into(),
        name: "Kimi K2 Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.15,
            output: 8.0,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_dot_5() -> Model {
    Model {
        id: "moonshotai/kimi-k2.5".into(),
        name: "Kimi K2.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 0.0,
        },
        context_window: 262_114.0,
        max_tokens: 262_114.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_dot_6() -> Model {
    Model {
        id: "moonshotai/kimi-k2.6".into(),
        name: "Kimi K2.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_87() -> [(&'static str, Model); 2] {
    [
        (
            "nvidia/nemotron-nano-12b-v2-vl",
            nvidia_nemotron_nano_12b_v2_vl(),
        ),
        ("nvidia/nemotron-nano-9b-v2", nvidia_nemotron_nano_9b_v2()),
    ]
}
/// Construct the recorded descriptor for this model.
fn nvidia_nemotron_nano_12b_v2_vl() -> Model {
    Model {
        id: "nvidia/nemotron-nano-12b-v2-vl".into(),
        name: "Nvidia Nemotron Nano 12B V2 VL".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.6,
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
fn nvidia_nemotron_nano_9b_v2() -> Model {
    Model {
        id: "nvidia/nemotron-nano-9b-v2".into(),
        name: "Nvidia Nemotron Nano 9B V2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.229_999_999_999_999_98,
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
pub(super) fn models_89() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-4-turbo", openai_gpt_4_turbo()),
        ("openai/gpt-4.1", openai_gpt_4_dot_1()),
        ("openai/gpt-4.1-mini", openai_gpt_4_dot_1_mini()),
        ("openai/gpt-4.1-nano", openai_gpt_4_dot_1_nano()),
        ("openai/gpt-4o", openai_gpt_4o()),
        ("openai/gpt-4o-mini", openai_gpt_4o_mini()),
        ("openai/gpt-5", openai_gpt_5()),
        ("openai/gpt-5-chat", openai_gpt_5_chat()),
        ("openai/gpt-5-codex", openai_gpt_5_codex()),
        ("openai/gpt-5-mini", openai_gpt_5_mini()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_99() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-5-nano", openai_gpt_5_nano()),
        ("openai/gpt-5-pro", openai_gpt_5_pro()),
        ("openai/gpt-5.1-codex", openai_gpt_5_dot_1_codex()),
        ("openai/gpt-5.1-codex-max", openai_gpt_5_dot_1_codex_max()),
        ("openai/gpt-5.1-codex-mini", openai_gpt_5_dot_1_codex_mini()),
        ("openai/gpt-5.1-instant", openai_gpt_5_dot_1_instant()),
        ("openai/gpt-5.1-thinking", openai_gpt_5_dot_1_thinking()),
        ("openai/gpt-5.2", openai_gpt_5_dot_2()),
        ("openai/gpt-5.2-chat", openai_gpt_5_dot_2_chat()),
        ("openai/gpt-5.2-codex", openai_gpt_5_dot_2_codex()),
    ]
}
/// Construct the recorded descriptor for this model.
fn openai_gpt_4_turbo() -> Model {
    Model {
        id: "openai/gpt-4-turbo".into(),
        name: "GPT-4 Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 10.0,
            output: 30.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4_dot_1() -> Model {
    Model {
        id: "openai/gpt-4.1".into(),
        name: "GPT-4.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4_dot_1_mini() -> Model {
    Model {
        id: "openai/gpt-4.1-mini".into(),
        name: "GPT-4.1 mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 1.599_999_999_999_999_9,
            cache_read: 0.099_999_999_999_999_99,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4_dot_1_nano() -> Model {
    Model {
        id: "openai/gpt-4.1-nano".into(),
        name: "GPT-4.1 nano".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 1_047_576.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4o() -> Model {
    Model {
        id: "openai/gpt-4o".into(),
        name: "GPT-4o".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4o_mini() -> Model {
    Model {
        id: "openai/gpt-4o-mini".into(),
        name: "GPT-4o mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5() -> Model {
    Model {
        id: "openai/gpt-5".into(),
        name: "GPT-5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_chat() -> Model {
    Model {
        id: "openai/gpt-5-chat".into(),
        name: "GPT 5 Chat".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_codex() -> Model {
    Model {
        id: "openai/gpt-5-codex".into(),
        name: "GPT-5-Codex".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_mini() -> Model {
    Model {
        id: "openai/gpt-5-mini".into(),
        name: "GPT-5 mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_nano() -> Model {
    Model {
        id: "openai/gpt-5-nano".into(),
        name: "GPT-5 nano".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.005,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_pro() -> Model {
    Model {
        id: "openai/gpt-5-pro".into(),
        name: "GPT-5 pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 120.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 272_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_1_codex() -> Model {
    Model {
        id: "openai/gpt-5.1-codex".into(),
        name: "GPT-5.1-Codex".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_1_codex_max() -> Model {
    Model {
        id: "openai/gpt-5.1-codex-max".into(),
        name: "GPT 5.1 Codex Max".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_1_codex_mini() -> Model {
    Model {
        id: "openai/gpt-5.1-codex-mini".into(),
        name: "GPT 5.1 Codex Mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_1_instant() -> Model {
    Model {
        id: "openai/gpt-5.1-instant".into(),
        name: "GPT-5.1 Instant".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_1_thinking() -> Model {
    Model {
        id: "openai/gpt-5.1-thinking".into(),
        name: "GPT 5.1 Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_2() -> Model {
    Model {
        id: "openai/gpt-5.2".into(),
        name: "GPT 5.2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_2_chat() -> Model {
    Model {
        id: "openai/gpt-5.2-chat".into(),
        name: "GPT 5.2 Chat".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_2_codex() -> Model {
    Model {
        id: "openai/gpt-5.2-codex".into(),
        name: "GPT 5.2 Codex".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_109() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-5.2-pro", openai_gpt_5_dot_2_pro()),
        ("openai/gpt-5.3-chat", openai_gpt_5_dot_3_chat()),
        ("openai/gpt-5.3-codex", openai_gpt_5_dot_3_codex()),
        ("openai/gpt-5.4", openai_gpt_5_dot_4()),
        ("openai/gpt-5.4-mini", openai_gpt_5_dot_4_mini()),
        ("openai/gpt-5.4-nano", openai_gpt_5_dot_4_nano()),
        ("openai/gpt-5.4-pro", openai_gpt_5_dot_4_pro()),
        ("openai/gpt-5.5", openai_gpt_5_dot_5()),
        ("openai/gpt-5.5-pro", openai_gpt_5_dot_5_pro()),
        ("openai/gpt-oss-20b", openai_gpt_oss_20b()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_119() -> [(&'static str, Model); 7] {
    [
        (
            "openai/gpt-oss-safeguard-20b",
            openai_gpt_oss_safeguard_20b(),
        ),
        ("openai/o1", openai_o1()),
        ("openai/o3", openai_o3()),
        ("openai/o3-deep-research", openai_o3_deep_research()),
        ("openai/o3-mini", openai_o3_mini()),
        ("openai/o3-pro", openai_o3_pro()),
        ("openai/o4-mini", openai_o4_mini()),
    ]
}
/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_2_pro() -> Model {
    Model {
        id: "openai/gpt-5.2-pro".into(),
        name: "GPT 5.2 ".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 21.0,
            output: 168.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_3_chat() -> Model {
    Model {
        id: "openai/gpt-5.3-chat".into(),
        name: "GPT-5.3 Chat".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_3_codex() -> Model {
    Model {
        id: "openai/gpt-5.3-codex".into(),
        name: "GPT 5.3 Codex".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_4() -> Model {
    Model {
        id: "openai/gpt-5.4".into(),
        name: "GPT 5.4".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_4_mini() -> Model {
    Model {
        id: "openai/gpt-5.4-mini".into(),
        name: "GPT 5.4 Mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.75,
            output: 4.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_4_nano() -> Model {
    Model {
        id: "openai/gpt-5.4-nano".into(),
        name: "GPT 5.4 Nano".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.25,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_4_pro() -> Model {
    Model {
        id: "openai/gpt-5.4-pro".into(),
        name: "GPT 5.4 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_5() -> Model {
    Model {
        id: "openai/gpt-5.5".into(),
        name: "GPT 5.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_5_pro() -> Model {
    Model {
        id: "openai/gpt-5.5-pro".into(),
        name: "GPT 5.5 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_20b() -> Model {
    Model {
        id: "openai/gpt-oss-20b".into(),
        name: "GPT OSS 120B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.199_999_999_999_999_98,
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
fn openai_gpt_oss_safeguard_20b() -> Model {
    Model {
        id: "openai/gpt-oss-safeguard-20b".into(),
        name: "GPT OSS Safeguard 20B".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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

/// Construct the recorded descriptor for this model.
fn openai_o1() -> Model {
    Model {
        id: "openai/o1".into(),
        name: "o1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 15.0,
            output: 60.0,
            cache_read: 7.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_o3() -> Model {
    Model {
        id: "openai/o3".into(),
        name: "o3".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_o3_deep_research() -> Model {
    Model {
        id: "openai/o3-deep-research".into(),
        name: "o3-deep-research".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 10.0,
            output: 40.0,
            cache_read: 2.5,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_o3_mini() -> Model {
    Model {
        id: "openai/o3-mini".into(),
        name: "o3-mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.55,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_o3_pro() -> Model {
    Model {
        id: "openai/o3-pro".into(),
        name: "o3 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 20.0,
            output: 80.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_o4_mini() -> Model {
    Model {
        id: "openai/o4-mini".into(),
        name: "o4-mini".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.1,
            output: 4.4,
            cache_read: 0.275,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_126() -> [(&'static str, Model); 2] {
    [
        ("perplexity/sonar", perplexity_sonar()),
        ("perplexity/sonar-pro", perplexity_sonar_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn perplexity_sonar() -> Model {
    Model {
        id: "perplexity/sonar".into(),
        name: "Sonar".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 127_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn perplexity_sonar_pro() -> Model {
    Model {
        id: "perplexity/sonar-pro".into(),
        name: "Sonar Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_128() -> [(&'static str, Model); 10] {
    [
        ("xai/grok-3", xai_grok_3()),
        ("xai/grok-3-fast", xai_grok_3_fast()),
        ("xai/grok-3-mini", xai_grok_3_mini()),
        ("xai/grok-3-mini-fast", xai_grok_3_mini_fast()),
        ("xai/grok-4", xai_grok_4()),
        (
            "xai/grok-4-fast-non-reasoning",
            xai_grok_4_fast_non_reasoning(),
        ),
        ("xai/grok-4-fast-reasoning", xai_grok_4_fast_reasoning()),
        (
            "xai/grok-4.1-fast-non-reasoning",
            xai_grok_4_dot_1_fast_non_reasoning(),
        ),
        (
            "xai/grok-4.1-fast-reasoning",
            xai_grok_4_dot_1_fast_reasoning(),
        ),
        ("xai/grok-4.20-multi-agent", xai_grok_4_dot_20_multi_agent()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_138() -> [(&'static str, Model); 7] {
    [
        (
            "xai/grok-4.20-multi-agent-beta",
            xai_grok_4_dot_20_multi_agent_beta(),
        ),
        (
            "xai/grok-4.20-non-reasoning",
            xai_grok_4_dot_20_non_reasoning(),
        ),
        (
            "xai/grok-4.20-non-reasoning-beta",
            xai_grok_4_dot_20_non_reasoning_beta(),
        ),
        ("xai/grok-4.20-reasoning", xai_grok_4_dot_20_reasoning()),
        (
            "xai/grok-4.20-reasoning-beta",
            xai_grok_4_dot_20_reasoning_beta(),
        ),
        ("xai/grok-4.3", xai_grok_4_dot_3()),
        ("xai/grok-code-fast-1", xai_grok_code_fast_1()),
    ]
}
/// Construct the recorded descriptor for this model.
fn xai_grok_3() -> Model {
    Model {
        id: "xai/grok-3".into(),
        name: "Grok 3 Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_3_fast() -> Model {
    Model {
        id: "xai/grok-3-fast".into(),
        name: "Grok 3 Fast Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 5.0,
            output: 25.0,
            cache_read: 1.25,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_3_mini() -> Model {
    Model {
        id: "xai/grok-3-mini".into(),
        name: "Grok 3 Mini Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_3_mini_fast() -> Model {
    Model {
        id: "xai/grok-3-mini-fast".into(),
        name: "Grok 3 Mini Fast Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 4.0,
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
fn xai_grok_4() -> Model {
    Model {
        id: "xai/grok-4".into(),
        name: "Grok 4".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.0,
            output: 15.0,
            cache_read: 0.75,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_fast_non_reasoning() -> Model {
    Model {
        id: "xai/grok-4-fast-non-reasoning".into(),
        name: "Grok 4 Fast Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_fast_reasoning() -> Model {
    Model {
        id: "xai/grok-4-fast-reasoning".into(),
        name: "Grok 4 Fast Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_1_fast_non_reasoning() -> Model {
    Model {
        id: "xai/grok-4.1-fast-non-reasoning".into(),
        name: "Grok 4.1 Fast Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_1_fast_reasoning() -> Model {
    Model {
        id: "xai/grok-4.1-fast-reasoning".into(),
        name: "Grok 4.1 Fast Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_20_multi_agent() -> Model {
    Model {
        id: "xai/grok-4.20-multi-agent".into(),
        name: "Grok 4.20 Multi-Agent".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_20_multi_agent_beta() -> Model {
    Model {
        id: "xai/grok-4.20-multi-agent-beta".into(),
        name: "Grok 4.20 Multi Agent Beta".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_20_non_reasoning() -> Model {
    Model {
        id: "xai/grok-4.20-non-reasoning".into(),
        name: "Grok 4.20 Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_20_non_reasoning_beta() -> Model {
    Model {
        id: "xai/grok-4.20-non-reasoning-beta".into(),
        name: "Grok 4.20 Beta Non-Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_20_reasoning() -> Model {
    Model {
        id: "xai/grok-4.20-reasoning".into(),
        name: "Grok 4.20 Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_20_reasoning_beta() -> Model {
    Model {
        id: "xai/grok-4.20-reasoning-beta".into(),
        name: "Grok 4.20 Beta Reasoning".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 2_000_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_4_dot_3() -> Model {
    Model {
        id: "xai/grok-4.3".into(),
        name: "Grok 4.3".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 2.5,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 1_000_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xai_grok_code_fast_1() -> Model {
    Model {
        id: "xai/grok-code-fast-1".into(),
        name: "Grok Code Fast 1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.5,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_145() -> [(&'static str, Model); 4] {
    [
        ("xiaomi/mimo-v2-flash", xiaomi_mimo_v2_flash()),
        ("xiaomi/mimo-v2-pro", xiaomi_mimo_v2_pro()),
        ("xiaomi/mimo-v2.5", xiaomi_mimo_v2_dot_5()),
        ("xiaomi/mimo-v2.5-pro", xiaomi_mimo_v2_dot_5_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_flash() -> Model {
    Model {
        id: "xiaomi/mimo-v2-flash".into(),
        name: "MiMo V2 Flash".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2-pro".into(),
        name: "MiMo V2 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_dot_5() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5".into(),
        name: "MiMo M2.5".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 131_100.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_dot_5_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5-pro".into(),
        name: "MiMo V2.5 Pro".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_149() -> [(&'static str, Model); 10] {
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
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_159() -> [(&'static str, Model); 3] {
    [
        ("zai/glm-5-turbo", zai_glm_5_turbo()),
        ("zai/glm-5.1", zai_glm_5_dot_1()),
        ("zai/glm-5v-turbo", zai_glm_5v_turbo()),
    ]
}
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
fn zai_glm_5_turbo() -> Model {
    Model {
        id: "zai/glm-5-turbo".into(),
        name: "GLM 5 Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 202_800.0,
        max_tokens: 131_100.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn zai_glm_5_dot_1() -> Model {
    Model {
        id: "zai/glm-5.1".into(),
        name: "GLM 5.1".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
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
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn zai_glm_5v_turbo() -> Model {
    Model {
        id: "zai/glm-5v-turbo".into(),
        name: "GLM 5V Turbo".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
