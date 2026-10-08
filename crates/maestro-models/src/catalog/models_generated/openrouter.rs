//! Generated model descriptors for openrouter.

use crate::{
    Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel, OpenAICompletionsCompat,
    ThinkingFormat,
};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    append_1(&mut models);
    append_2(&mut models);
    models
}
/// Append the next provider descriptor batches in recorded order.
fn append_1(models: &mut IndexMap<&'static str, Model>) {
    models.extend(models_0());
    models.extend(models_1());
    models.extend(models_2());
    models.extend(models_3());
    models.extend(models_8());
    models.extend(models_18());
    models.extend(models_22());
    models.extend(models_26());
    models.extend(models_27());
    models.extend(models_29());
    models.extend(models_33());
    models.extend(models_35());
    models.extend(models_45());
    models.extend(models_46());
    models.extend(models_56());
    models.extend(models_64());
    models.extend(models_65());
    models.extend(models_66());
    models.extend(models_68());
    models.extend(models_69());
    models.extend(models_75());
    models.extend(models_81());
    models.extend(models_91());
    models.extend(models_100());
    models.extend(models_102());
    models.extend(models_107());
    models.extend(models_108());
    models.extend(models_118());
    models.extend(models_128());
    models.extend(models_138());
    models.extend(models_148());
    models.extend(models_158());
    models.extend(models_168());
    models.extend(models_174());
    models.extend(models_177());
    models.extend(models_179());
    models.extend(models_180());
    models.extend(models_190());
    models.extend(models_199());
    models.extend(models_209());
    models.extend(models_218());
    models.extend(models_228());
    models.extend(models_229());
    models.extend(models_230());
    models.extend(models_232());
    models.extend(models_233());
    models.extend(models_234());
    models.extend(models_236());
    models.extend(models_237());
    models.extend(models_238());
}
/// Append the next provider descriptor batches in recorded order.
fn append_2(models: &mut IndexMap<&'static str, Model>) {
    models.extend(models_248());
    models.extend(models_253());
    models.extend(models_263());
    models.extend(models_266());
    models.extend(models_269());
    models.extend(models_270());
    models.extend(models_271());
    models.extend(models_272());
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [("ai21/jamba-large-1.7", ai21_jamba_large_1_dot_7())]
}
/// Construct the recorded descriptor for this model.
fn ai21_jamba_large_1_dot_7() -> Model {
    Model {
        id: "ai21/jamba-large-1.7".into(),
        name: "AI21: Jamba Large 1.7".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 8.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_1() -> [(&'static str, Model); 1] {
    [(
        "alibaba/tongyi-deepresearch-30b-a3b",
        alibaba_tongyi_deepresearch_30b_a3b(),
    )]
}
/// Construct the recorded descriptor for this model.
fn alibaba_tongyi_deepresearch_30b_a3b() -> Model {
    Model {
        id: "alibaba/tongyi-deepresearch-30b-a3b".into(),
        name: "Tongyi DeepResearch 30B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.09,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_2() -> [(&'static str, Model); 1] {
    [(
        "allenai/olmo-3.1-32b-instruct",
        allenai_olmo_3_dot_1_32b_instruct(),
    )]
}
/// Construct the recorded descriptor for this model.
fn allenai_olmo_3_dot_1_32b_instruct() -> Model {
    Model {
        id: "allenai/olmo-3.1-32b-instruct".into(),
        name: "AllenAI: Olmo 3.1 32B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 65_536.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_3() -> [(&'static str, Model); 5] {
    [
        ("amazon/nova-2-lite-v1", amazon_nova_2_lite_v1()),
        ("amazon/nova-lite-v1", amazon_nova_lite_v1()),
        ("amazon/nova-micro-v1", amazon_nova_micro_v1()),
        ("amazon/nova-premier-v1", amazon_nova_premier_v1()),
        ("amazon/nova-pro-v1", amazon_nova_pro_v1()),
    ]
}
/// Construct the recorded descriptor for this model.
fn amazon_nova_2_lite_v1() -> Model {
    Model {
        id: "amazon/nova-2-lite-v1".into(),
        name: "Amazon: Nova 2 Lite".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn amazon_nova_lite_v1() -> Model {
    Model {
        id: "amazon/nova-lite-v1".into(),
        name: "Amazon: Nova Lite 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 5120.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn amazon_nova_micro_v1() -> Model {
    Model {
        id: "amazon/nova-micro-v1".into(),
        name: "Amazon: Nova Micro 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.035,
            output: 0.14,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 5120.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn amazon_nova_premier_v1() -> Model {
    Model {
        id: "amazon/nova-premier-v1".into(),
        name: "Amazon: Nova Premier 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 12.5,
            cache_read: 0.625,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn amazon_nova_pro_v1() -> Model {
    Model {
        id: "amazon/nova-pro-v1".into(),
        name: "Amazon: Nova Pro 1.0".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.799_999_999_999_999_9,
            output: 3.199_999_999_999_999_7,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 5120.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_8() -> [(&'static str, Model); 10] {
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
            "anthropic/claude-3.7-sonnet:thinking",
            anthropic_claude_3_dot_7_sonnet_thinking(),
        ),
        (
            "anthropic/claude-haiku-4.5",
            anthropic_claude_haiku_4_dot_5(),
        ),
        ("anthropic/claude-opus-4", anthropic_claude_opus_4()),
        ("anthropic/claude-opus-4.1", anthropic_claude_opus_4_dot_1()),
        ("anthropic/claude-opus-4.5", anthropic_claude_opus_4_dot_5()),
        ("anthropic/claude-opus-4.6", anthropic_claude_opus_4_dot_6()),
        (
            "anthropic/claude-opus-4.6-fast",
            anthropic_claude_opus_4_dot_6_fast(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_18() -> [(&'static str, Model); 4] {
    [
        ("anthropic/claude-opus-4.7", anthropic_claude_opus_4_dot_7()),
        ("anthropic/claude-sonnet-4", anthropic_claude_sonnet_4()),
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
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_266() -> [(&'static str, Model); 3] {
    [
        (
            "~anthropic/claude-haiku-latest",
            anthropic_claude_haiku_latest(),
        ),
        (
            "~anthropic/claude-opus-latest",
            anthropic_claude_opus_latest(),
        ),
        (
            "~anthropic/claude-sonnet-latest",
            anthropic_claude_sonnet_latest(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn anthropic_claude_3_haiku() -> Model {
    Model {
        id: "anthropic/claude-3-haiku".into(),
        name: "Anthropic: Claude 3 Haiku".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude 3.5 Haiku".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude 3.7 Sonnet".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_3_dot_7_sonnet_thinking() -> Model {
    Model {
        id: "anthropic/claude-3.7-sonnet:thinking".into(),
        name: "Anthropic: Claude 3.7 Sonnet (thinking)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_claude_haiku_4_dot_5() -> Model {
    Model {
        id: "anthropic/claude-haiku-4.5".into(),
        name: "Anthropic: Claude Haiku 4.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude Opus 4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude Opus 4.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude Opus 4.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude Opus 4.6".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn anthropic_claude_opus_4_dot_6_fast() -> Model {
    Model {
        id: "anthropic/claude-opus-4.6-fast".into(),
        name: "Anthropic: Claude Opus 4.6 (Fast)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 150.0,
            cache_read: 3.0,
            cache_write: 37.5,
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
        name: "Anthropic: Claude Opus 4.7".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude Sonnet 4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude Sonnet 4.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "Anthropic: Claude Sonnet 4.6".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

/// Construct the recorded descriptor for this model.
fn anthropic_claude_haiku_latest() -> Model {
    Model {
        id: "~anthropic/claude-haiku-latest".into(),
        name: "Anthropic Claude Haiku Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn anthropic_claude_opus_latest() -> Model {
    Model {
        id: "~anthropic/claude-opus-latest".into(),
        name: "Anthropic: Claude Opus Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
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
fn anthropic_claude_sonnet_latest() -> Model {
    Model {
        id: "~anthropic/claude-sonnet-latest".into(),
        name: "Anthropic Claude Sonnet Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
pub(super) fn models_22() -> [(&'static str, Model); 4] {
    [
        (
            "arcee-ai/trinity-large-preview",
            arcee_ai_trinity_large_preview(),
        ),
        (
            "arcee-ai/trinity-large-thinking",
            arcee_ai_trinity_large_thinking(),
        ),
        ("arcee-ai/trinity-mini", arcee_ai_trinity_mini()),
        ("arcee-ai/virtuoso-large", arcee_ai_virtuoso_large()),
    ]
}
/// Construct the recorded descriptor for this model.
fn arcee_ai_trinity_large_preview() -> Model {
    Model {
        id: "arcee-ai/trinity-large-preview".into(),
        name: "Arcee AI: Trinity Large Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn arcee_ai_trinity_large_thinking() -> Model {
    Model {
        id: "arcee-ai/trinity-large-thinking".into(),
        name: "Arcee AI: Trinity Large Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.85,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn arcee_ai_trinity_mini() -> Model {
    Model {
        id: "arcee-ai/trinity-mini".into(),
        name: "Arcee AI: Trinity Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.045,
            output: 0.15,
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
fn arcee_ai_virtuoso_large() -> Model {
    Model {
        id: "arcee-ai/virtuoso-large".into(),
        name: "Arcee AI: Virtuoso Large".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.75,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_26() -> [(&'static str, Model); 1] {
    [("auto", auto())]
}
/// Construct the recorded descriptor for this model.
fn auto() -> Model {
    Model {
        id: "auto".into(),
        name: "Auto".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 30_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_27() -> [(&'static str, Model); 2] {
    [
        ("baidu/ernie-4.5-21b-a3b", baidu_ernie_4_dot_5_21b_a3b()),
        (
            "baidu/ernie-4.5-vl-28b-a3b",
            baidu_ernie_4_dot_5_vl_28b_a3b(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn baidu_ernie_4_dot_5_21b_a3b() -> Model {
    Model {
        id: "baidu/ernie-4.5-21b-a3b".into(),
        name: "Baidu: ERNIE 4.5 21B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.28,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 120_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn baidu_ernie_4_dot_5_vl_28b_a3b() -> Model {
    Model {
        id: "baidu/ernie-4.5-vl-28b-a3b".into(),
        name: "Baidu: ERNIE 4.5 VL 28B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.14,
            output: 0.56,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 30_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_29() -> [(&'static str, Model); 4] {
    [
        ("bytedance-seed/seed-1.6", bytedance_seed_seed_1_dot_6()),
        (
            "bytedance-seed/seed-1.6-flash",
            bytedance_seed_seed_1_dot_6_flash(),
        ),
        (
            "bytedance-seed/seed-2.0-lite",
            bytedance_seed_seed_2_dot_0_lite(),
        ),
        (
            "bytedance-seed/seed-2.0-mini",
            bytedance_seed_seed_2_dot_0_mini(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn bytedance_seed_seed_1_dot_6() -> Model {
    Model {
        id: "bytedance-seed/seed-1.6".into(),
        name: "ByteDance Seed: Seed 1.6".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn bytedance_seed_seed_1_dot_6_flash() -> Model {
    Model {
        id: "bytedance-seed/seed-1.6-flash".into(),
        name: "ByteDance Seed: Seed 1.6 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn bytedance_seed_seed_2_dot_0_lite() -> Model {
    Model {
        id: "bytedance-seed/seed-2.0-lite".into(),
        name: "ByteDance Seed: Seed-2.0-Lite".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
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
fn bytedance_seed_seed_2_dot_0_mini() -> Model {
    Model {
        id: "bytedance-seed/seed-2.0-mini".into(),
        name: "ByteDance Seed: Seed-2.0-Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
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
pub(super) fn models_33() -> [(&'static str, Model); 2] {
    [
        ("cohere/command-r-08-2024", cohere_command_r_08_2024()),
        (
            "cohere/command-r-plus-08-2024",
            cohere_command_r_plus_08_2024(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn cohere_command_r_08_2024() -> Model {
    Model {
        id: "cohere/command-r-08-2024".into(),
        name: "Cohere: Command R (08-2024)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
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
fn cohere_command_r_plus_08_2024() -> Model {
    Model {
        id: "cohere/command-r-plus-08-2024".into(),
        name: "Cohere: Command R+ (08-2024)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
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
pub(super) fn models_35() -> [(&'static str, Model); 10] {
    [
        ("deepseek/deepseek-chat", deepseek_deepseek_chat()),
        (
            "deepseek/deepseek-chat-v3-0324",
            deepseek_deepseek_chat_v3_0324(),
        ),
        (
            "deepseek/deepseek-chat-v3.1",
            deepseek_deepseek_chat_v3_dot_1(),
        ),
        ("deepseek/deepseek-r1", deepseek_deepseek_r1()),
        ("deepseek/deepseek-r1-0528", deepseek_deepseek_r1_0528()),
        (
            "deepseek/deepseek-v3.1-terminus",
            deepseek_deepseek_v3_dot_1_terminus(),
        ),
        ("deepseek/deepseek-v3.2", deepseek_deepseek_v3_dot_2()),
        (
            "deepseek/deepseek-v3.2-exp",
            deepseek_deepseek_v3_dot_2_exp(),
        ),
        ("deepseek/deepseek-v4-flash", deepseek_deepseek_v4_flash()),
        ("deepseek/deepseek-v4-pro", deepseek_deepseek_v4_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_chat() -> Model {
    Model {
        id: "deepseek/deepseek-chat".into(),
        name: "DeepSeek: DeepSeek V3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.32,
            output: 0.889_999_999_999_999_9,
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
fn deepseek_deepseek_chat_v3_0324() -> Model {
    Model {
        id: "deepseek/deepseek-chat-v3-0324".into(),
        name: "DeepSeek: DeepSeek V3 0324".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.77,
            cache_read: 0.135,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_chat_v3_dot_1() -> Model {
    Model {
        id: "deepseek/deepseek-chat-v3.1".into(),
        name: "DeepSeek: DeepSeek V3.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 7168.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_r1() -> Model {
    Model {
        id: "deepseek/deepseek-r1".into(),
        name: "DeepSeek: R1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.7,
            output: 2.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 64_000.0,
        max_tokens: 16_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_r1_0528() -> Model {
    Model {
        id: "deepseek/deepseek-r1-0528".into(),
        name: "DeepSeek: R1 0528".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 2.150_000_000_000_000_4,
            cache_read: 0.35,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3_dot_1_terminus() -> Model {
    Model {
        id: "deepseek/deepseek-v3.1-terminus".into(),
        name: "DeepSeek: DeepSeek V3.1 Terminus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.27,
            output: 0.95,
            cache_read: 0.13,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3_dot_2() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2".into(),
        name: "DeepSeek: DeepSeek V3.2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.252,
            output: 0.378,
            cache_read: 0.0252,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v3_dot_2_exp() -> Model {
    Model {
        id: "deepseek/deepseek-v3.2-exp".into(),
        name: "DeepSeek: DeepSeek V3.2 Exp".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.27,
            output: 0.41,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v4_flash() -> Model {
    Model {
        id: "deepseek/deepseek-v4-flash".into(),
        name: "DeepSeek: DeepSeek V4 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, None),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("high".into())),
                (ModelThinkingLevel::Xhigh, Some("max".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.14,
            output: 0.28,
            cache_read: 0.0028,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 384_000.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                requires_reasoning_content_on_assistant_messages: Some(true),
                thinking_format: Some(ThinkingFormat::Deepseek),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek/deepseek-v4-pro".into(),
        name: "DeepSeek: DeepSeek V4 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, None),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("high".into())),
                (ModelThinkingLevel::Xhigh, Some("max".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                requires_reasoning_content_on_assistant_messages: Some(true),
                thinking_format: Some(ThinkingFormat::Deepseek),
                ..Default::default()
            },
        ))),
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_45() -> [(&'static str, Model); 1] {
    [("essentialai/rnj-1-instruct", essentialai_rnj_1_instruct())]
}
/// Construct the recorded descriptor for this model.
fn essentialai_rnj_1_instruct() -> Model {
    Model {
        id: "essentialai/rnj-1-instruct".into(),
        name: "EssentialAI: Rnj 1 Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_46() -> [(&'static str, Model); 10] {
    [
        (
            "google/gemini-2.0-flash-001",
            google_gemini_2_dot_0_flash_001(),
        ),
        (
            "google/gemini-2.0-flash-lite-001",
            google_gemini_2_dot_0_flash_lite_001(),
        ),
        ("google/gemini-2.5-flash", google_gemini_2_dot_5_flash()),
        (
            "google/gemini-2.5-flash-lite",
            google_gemini_2_dot_5_flash_lite(),
        ),
        (
            "google/gemini-2.5-flash-lite-preview-09-2025",
            google_gemini_2_dot_5_flash_lite_preview_09_2025(),
        ),
        ("google/gemini-2.5-pro", google_gemini_2_dot_5_pro()),
        (
            "google/gemini-2.5-pro-preview",
            google_gemini_2_dot_5_pro_preview(),
        ),
        (
            "google/gemini-2.5-pro-preview-05-06",
            google_gemini_2_dot_5_pro_preview_05_06(),
        ),
        (
            "google/gemini-3-flash-preview",
            google_gemini_3_flash_preview(),
        ),
        (
            "google/gemini-3.1-flash-lite-preview",
            google_gemini_3_dot_1_flash_lite_preview(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_56() -> [(&'static str, Model); 8] {
    [
        (
            "google/gemini-3.1-pro-preview",
            google_gemini_3_dot_1_pro_preview(),
        ),
        (
            "google/gemini-3.1-pro-preview-customtools",
            google_gemini_3_dot_1_pro_preview_customtools(),
        ),
        ("google/gemma-3-12b-it", google_gemma_3_12b_it()),
        ("google/gemma-3-27b-it", google_gemma_3_27b_it()),
        ("google/gemma-4-26b-a4b-it", google_gemma_4_26b_a4b_it()),
        (
            "google/gemma-4-26b-a4b-it:free",
            google_gemma_4_26b_a4b_it_free(),
        ),
        ("google/gemma-4-31b-it", google_gemma_4_31b_it()),
        ("google/gemma-4-31b-it:free", google_gemma_4_31b_it_free()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_269() -> [(&'static str, Model); 1] {
    [("~google/gemini-flash-latest", google_gemini_flash_latest())]
}
/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_0_flash_001() -> Model {
    Model {
        id: "google/gemini-2.0-flash-001".into(),
        name: "Google: Gemini 2.0 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_0_flash_lite_001() -> Model {
    Model {
        id: "google/gemini-2.0-flash-lite-001".into(),
        name: "Google: Gemini 2.0 Flash Lite".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.3,
            cache_read: 0.0,
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
        name: "Google: Gemini 2.5 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 2.5,
            cache_read: 0.03,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_flash_lite() -> Model {
    Model {
        id: "google/gemini-2.5-flash-lite".into(),
        name: "Google: Gemini 2.5 Flash Lite".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_flash_lite_preview_09_2025() -> Model {
    Model {
        id: "google/gemini-2.5-flash-lite-preview-09-2025".into(),
        name: "Google: Gemini 2.5 Flash Lite Preview 09-2025".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_pro() -> Model {
    Model {
        id: "google/gemini-2.5-pro".into(),
        name: "Google: Gemini 2.5 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_pro_preview() -> Model {
    Model {
        id: "google/gemini-2.5-pro-preview".into(),
        name: "Google: Gemini 2.5 Pro Preview 06-05".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_2_dot_5_pro_preview_05_06() -> Model {
    Model {
        id: "google/gemini-2.5-pro-preview-05-06".into(),
        name: "Google: Gemini 2.5 Pro Preview 05-06".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_flash_preview() -> Model {
    Model {
        id: "google/gemini-3-flash-preview".into(),
        name: "Google: Gemini 3 Flash Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_dot_1_flash_lite_preview() -> Model {
    Model {
        id: "google/gemini-3.1-flash-lite-preview".into(),
        name: "Google: Gemini 3.1 Flash Lite Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_dot_1_pro_preview() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview".into(),
        name: "Google: Gemini 3.1 Pro Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_3_dot_1_pro_preview_customtools() -> Model {
    Model {
        id: "google/gemini-3.1-pro-preview-customtools".into(),
        name: "Google: Gemini 3.1 Pro Preview Custom Tools".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemma_3_12b_it() -> Model {
    Model {
        id: "google/gemma-3-12b-it".into(),
        name: "Google: Gemma 3 12B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.04,
            output: 0.13,
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
fn google_gemma_3_27b_it() -> Model {
    Model {
        id: "google/gemma-3-27b-it".into(),
        name: "Google: Gemma 3 27B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.08,
            output: 0.16,
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
fn google_gemma_4_26b_a4b_it() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it".into(),
        name: "Google: Gemma 4 26B A4B ".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.06,
            output: 0.33,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemma_4_26b_a4b_it_free() -> Model {
    Model {
        id: "google/gemma-4-26b-a4b-it:free".into(),
        name: "Google: Gemma 4 26B A4B  (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemma_4_31b_it() -> Model {
    Model {
        id: "google/gemma-4-31b-it".into(),
        name: "Google: Gemma 4 31B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 0.38,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemma_4_31b_it_free() -> Model {
    Model {
        id: "google/gemma-4-31b-it:free".into(),
        name: "Google: Gemma 4 31B (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_gemini_flash_latest() -> Model {
    Model {
        id: "~google/gemini-flash-latest".into(),
        name: "Google Gemini Flash Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.083_333_333_333_333_34,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_270() -> [(&'static str, Model); 1] {
    [("~google/gemini-pro-latest", google_gemini_pro_latest())]
}
/// Construct the recorded descriptor for this model.
fn google_gemini_pro_latest() -> Model {
    Model {
        id: "~google/gemini-pro-latest".into(),
        name: "Google Gemini Pro Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.375,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_64() -> [(&'static str, Model); 1] {
    [(
        "ibm-granite/granite-4.1-8b",
        ibm_granite_granite_4_dot_1_8b(),
    )]
}
/// Construct the recorded descriptor for this model.
fn ibm_granite_granite_4_dot_1_8b() -> Model {
    Model {
        id: "ibm-granite/granite-4.1-8b".into(),
        name: "IBM: Granite 4.1 8B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_65() -> [(&'static str, Model); 1] {
    [("inception/mercury-2", inception_mercury_2())]
}
/// Construct the recorded descriptor for this model.
fn inception_mercury_2() -> Model {
    Model {
        id: "inception/mercury-2".into(),
        name: "Inception: Mercury 2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 50_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_66() -> [(&'static str, Model); 2] {
    [
        (
            "inclusionai/ling-2.6-1t:free",
            inclusionai_ling_2_dot_6_1t_free(),
        ),
        (
            "inclusionai/ling-2.6-flash",
            inclusionai_ling_2_dot_6_flash(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn inclusionai_ling_2_dot_6_1t_free() -> Model {
    Model {
        id: "inclusionai/ling-2.6-1t:free".into(),
        name: "inclusionAI: Ling-2.6-1T (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn inclusionai_ling_2_dot_6_flash() -> Model {
    Model {
        id: "inclusionai/ling-2.6-flash".into(),
        name: "inclusionAI: Ling-2.6-flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.24,
            cache_read: 0.016,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_68() -> [(&'static str, Model); 1] {
    [("kwaipilot/kat-coder-pro-v2", kwaipilot_kat_coder_pro_v2())]
}
/// Construct the recorded descriptor for this model.
fn kwaipilot_kat_coder_pro_v2() -> Model {
    Model {
        id: "kwaipilot/kat-coder-pro-v2".into(),
        name: "Kwaipilot: KAT-Coder-Pro V2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 80_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_69() -> [(&'static str, Model); 6] {
    [
        (
            "meta-llama/llama-3-8b-instruct",
            meta_llama_llama_3_8b_instruct(),
        ),
        (
            "meta-llama/llama-3.1-70b-instruct",
            meta_llama_llama_3_dot_1_70b_instruct(),
        ),
        (
            "meta-llama/llama-3.1-8b-instruct",
            meta_llama_llama_3_dot_1_8b_instruct(),
        ),
        (
            "meta-llama/llama-3.3-70b-instruct",
            meta_llama_llama_3_dot_3_70b_instruct(),
        ),
        (
            "meta-llama/llama-3.3-70b-instruct:free",
            meta_llama_llama_3_dot_3_70b_instruct_free(),
        ),
        ("meta-llama/llama-4-scout", meta_llama_llama_4_scout()),
    ]
}
/// Construct the recorded descriptor for this model.
fn meta_llama_llama_3_8b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3-8b-instruct".into(),
        name: "Meta: Llama 3 8B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.03,
            output: 0.04,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_llama_3_dot_1_70b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3.1-70b-instruct".into(),
        name: "Meta: Llama 3.1 70B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 0.399_999_999_999_999_97,
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
fn meta_llama_llama_3_dot_1_8b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3.1-8b-instruct".into(),
        name: "Meta: Llama 3.1 8B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.02,
            output: 0.049_999_999_999_999_996,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_384.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_llama_3_dot_3_70b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3.3-70b-instruct".into(),
        name: "Meta: Llama 3.3 70B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.32,
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
fn meta_llama_llama_3_dot_3_70b_instruct_free() -> Model {
    Model {
        id: "meta-llama/llama-3.3-70b-instruct:free".into(),
        name: "Meta: Llama 3.3 70B Instruct (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 65_536.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_llama_llama_4_scout() -> Model {
    Model {
        id: "meta-llama/llama-4-scout".into(),
        name: "Meta: Llama 4 Scout".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.08,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 327_680.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_75() -> [(&'static str, Model); 6] {
    [
        ("minimax/minimax-m1", minimax_minimax_m1()),
        ("minimax/minimax-m2", minimax_minimax_m2()),
        ("minimax/minimax-m2.1", minimax_minimax_m2_dot_1()),
        ("minimax/minimax-m2.5", minimax_minimax_m2_dot_5()),
        ("minimax/minimax-m2.5:free", minimax_minimax_m2_dot_5_free()),
        ("minimax/minimax-m2.7", minimax_minimax_m2_dot_7()),
    ]
}
/// Construct the recorded descriptor for this model.
fn minimax_minimax_m1() -> Model {
    Model {
        id: "minimax/minimax-m1".into(),
        name: "MiniMax: MiniMax M1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2() -> Model {
    Model {
        id: "minimax/minimax-m2".into(),
        name: "MiniMax: MiniMax M2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.255,
            output: 1.0,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 196_608.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_1() -> Model {
    Model {
        id: "minimax/minimax-m2.1".into(),
        name: "MiniMax: MiniMax M2.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.29,
            output: 0.95,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 196_608.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax/minimax-m2.5".into(),
        name: "MiniMax: MiniMax M2.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 1.15,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_5_free() -> Model {
    Model {
        id: "minimax/minimax-m2.5:free".into(),
        name: "MiniMax: MiniMax M2.5 (free)".into(),
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
        context_window: 196_608.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_minimax_m2_dot_7() -> Model {
    Model {
        id: "minimax/minimax-m2.7".into(),
        name: "MiniMax: MiniMax M2.7".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.059,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_81() -> [(&'static str, Model); 10] {
    [
        ("mistralai/codestral-2508", mistralai_codestral_2508()),
        ("mistralai/devstral-2512", mistralai_devstral_2512()),
        ("mistralai/devstral-medium", mistralai_devstral_medium()),
        ("mistralai/devstral-small", mistralai_devstral_small()),
        (
            "mistralai/ministral-14b-2512",
            mistralai_ministral_14b_2512(),
        ),
        ("mistralai/ministral-3b-2512", mistralai_ministral_3b_2512()),
        ("mistralai/ministral-8b-2512", mistralai_ministral_8b_2512()),
        ("mistralai/mistral-large", mistralai_mistral_large()),
        (
            "mistralai/mistral-large-2407",
            mistralai_mistral_large_2407(),
        ),
        (
            "mistralai/mistral-large-2411",
            mistralai_mistral_large_2411(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_91() -> [(&'static str, Model); 9] {
    [
        (
            "mistralai/mistral-large-2512",
            mistralai_mistral_large_2512(),
        ),
        ("mistralai/mistral-medium-3", mistralai_mistral_medium_3()),
        (
            "mistralai/mistral-medium-3.1",
            mistralai_mistral_medium_3_dot_1(),
        ),
        ("mistralai/mistral-nemo", mistralai_mistral_nemo()),
        ("mistralai/mistral-saba", mistralai_mistral_saba()),
        (
            "mistralai/mistral-small-2603",
            mistralai_mistral_small_2603(),
        ),
        (
            "mistralai/mistral-small-3.2-24b-instruct",
            mistralai_mistral_small_3_dot_2_24b_instruct(),
        ),
        (
            "mistralai/mixtral-8x22b-instruct",
            mistralai_mixtral_8x22b_instruct(),
        ),
        (
            "mistralai/mixtral-8x7b-instruct",
            mistralai_mixtral_8x7b_instruct(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn mistralai_codestral_2508() -> Model {
    Model {
        id: "mistralai/codestral-2508".into(),
        name: "Mistral: Codestral 2508".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.899_999_999_999_999_9,
            cache_read: 0.03,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_devstral_2512() -> Model {
    Model {
        id: "mistralai/devstral-2512".into(),
        name: "Mistral: Devstral 2 2512".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_devstral_medium() -> Model {
    Model {
        id: "mistralai/devstral-medium".into(),
        name: "Mistral: Devstral Medium".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_devstral_small() -> Model {
    Model {
        id: "mistralai/devstral-small".into(),
        name: "Mistral: Devstral Small 1.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_ministral_14b_2512() -> Model {
    Model {
        id: "mistralai/ministral-14b-2512".into(),
        name: "Mistral: Ministral 3 14B 2512".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.199_999_999_999_999_98,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_ministral_3b_2512() -> Model {
    Model {
        id: "mistralai/ministral-3b-2512".into(),
        name: "Mistral: Ministral 3 3B 2512".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_ministral_8b_2512() -> Model {
    Model {
        id: "mistralai/ministral-8b-2512".into(),
        name: "Mistral: Ministral 3 8B 2512".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_large() -> Model {
    Model {
        id: "mistralai/mistral-large".into(),
        name: "Mistral Large".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_large_2407() -> Model {
    Model {
        id: "mistralai/mistral-large-2407".into(),
        name: "Mistral Large 2407".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_large_2411() -> Model {
    Model {
        id: "mistralai/mistral-large-2411".into(),
        name: "Mistral Large 2411".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_large_2512() -> Model {
    Model {
        id: "mistralai/mistral-large-2512".into(),
        name: "Mistral: Mistral Large 3 2512".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_medium_3() -> Model {
    Model {
        id: "mistralai/mistral-medium-3".into(),
        name: "Mistral: Mistral Medium 3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_medium_3_dot_1() -> Model {
    Model {
        id: "mistralai/mistral-medium-3.1".into(),
        name: "Mistral: Mistral Medium 3.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_nemo() -> Model {
    Model {
        id: "mistralai/mistral-nemo".into(),
        name: "Mistral: Mistral Nemo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.02,
            output: 0.03,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_saba() -> Model {
    Model {
        id: "mistralai/mistral-saba".into(),
        name: "Mistral: Saba".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.6,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_small_2603() -> Model {
    Model {
        id: "mistralai/mistral-small-2603".into(),
        name: "Mistral: Mistral Small 4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mistral_small_3_dot_2_24b_instruct() -> Model {
    Model {
        id: "mistralai/mistral-small-3.2-24b-instruct".into(),
        name: "Mistral: Mistral Small 3.2 24B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.199_999_999_999_999_98,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mixtral_8x22b_instruct() -> Model {
    Model {
        id: "mistralai/mixtral-8x22b-instruct".into(),
        name: "Mistral: Mixtral 8x22B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 65_536.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_mixtral_8x7b_instruct() -> Model {
    Model {
        id: "mistralai/mixtral-8x7b-instruct".into(),
        name: "Mistral: Mixtral 8x7B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.54,
            output: 0.54,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_100() -> [(&'static str, Model); 2] {
    [
        (
            "mistralai/pixtral-large-2411",
            mistralai_pixtral_large_2411(),
        ),
        (
            "mistralai/voxtral-small-24b-2507",
            mistralai_voxtral_small_24b_2507(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn mistralai_pixtral_large_2411() -> Model {
    Model {
        id: "mistralai/pixtral-large-2411".into(),
        name: "Mistral: Pixtral Large 2411".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistralai_voxtral_small_24b_2507() -> Model {
    Model {
        id: "mistralai/voxtral-small-24b-2507".into(),
        name: "Mistral: Voxtral Small 24B 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_102() -> [(&'static str, Model); 5] {
    [
        ("moonshotai/kimi-k2", moonshotai_kimi_k2()),
        ("moonshotai/kimi-k2-0905", moonshotai_kimi_k2_0905()),
        ("moonshotai/kimi-k2-thinking", moonshotai_kimi_k2_thinking()),
        ("moonshotai/kimi-k2.5", moonshotai_kimi_k2_dot_5()),
        ("moonshotai/kimi-k2.6", moonshotai_kimi_k2_dot_6()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_271() -> [(&'static str, Model); 1] {
    [("~moonshotai/kimi-latest", moonshotai_kimi_latest())]
}
/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2() -> Model {
    Model {
        id: "moonshotai/kimi-k2".into(),
        name: "MoonshotAI: Kimi K2 0711".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_0905() -> Model {
    Model {
        id: "moonshotai/kimi-k2-0905".into(),
        name: "MoonshotAI: Kimi K2 0905".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_thinking() -> Model {
    Model {
        id: "moonshotai/kimi-k2-thinking".into(),
        name: "MoonshotAI: Kimi K2 Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_dot_5() -> Model {
    Model {
        id: "moonshotai/kimi-k2.5".into(),
        name: "MoonshotAI: Kimi K2.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.41,
            output: 2.06,
            cache_read: 0.07,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_k2_dot_6() -> Model {
    Model {
        id: "moonshotai/kimi-k2.6".into(),
        name: "MoonshotAI: Kimi K2.6".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.74,
            output: 3.49,
            cache_read: 0.14,
            cache_write: 0.0,
        },
        context_window: 262_142.0,
        max_tokens: 262_142.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn moonshotai_kimi_latest() -> Model {
    Model {
        id: "~moonshotai/kimi-latest".into(),
        name: "MoonshotAI Kimi Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.74,
            output: 3.49,
            cache_read: 0.14,
            cache_write: 0.0,
        },
        context_window: 262_142.0,
        max_tokens: 262_142.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_107() -> [(&'static str, Model); 1] {
    [(
        "nex-agi/deepseek-v3.1-nex-n1",
        nex_agi_deepseek_v3_dot_1_nex_n1(),
    )]
}
/// Construct the recorded descriptor for this model.
fn nex_agi_deepseek_v3_dot_1_nex_n1() -> Model {
    Model {
        id: "nex-agi/deepseek-v3.1-nex-n1".into(),
        name: "Nex AGI: DeepSeek V3.1 Nex N1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.135,
            output: 0.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 163_840.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_108() -> [(&'static str, Model); 10] {
    [
        (
            "nvidia/llama-3.1-nemotron-70b-instruct",
            nvidia_llama_3_dot_1_nemotron_70b_instruct(),
        ),
        (
            "nvidia/llama-3.3-nemotron-super-49b-v1.5",
            nvidia_llama_3_dot_3_nemotron_super_49b_v1_dot_5(),
        ),
        (
            "nvidia/nemotron-3-nano-30b-a3b",
            nvidia_nemotron_3_nano_30b_a3b(),
        ),
        (
            "nvidia/nemotron-3-nano-30b-a3b:free",
            nvidia_nemotron_3_nano_30b_a3b_free(),
        ),
        (
            "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free",
            nvidia_nemotron_3_nano_omni_30b_a3b_reasoning_free(),
        ),
        (
            "nvidia/nemotron-3-super-120b-a12b",
            nvidia_nemotron_3_super_120b_a12b(),
        ),
        (
            "nvidia/nemotron-3-super-120b-a12b:free",
            nvidia_nemotron_3_super_120b_a12b_free(),
        ),
        (
            "nvidia/nemotron-nano-12b-v2-vl:free",
            nvidia_nemotron_nano_12b_v2_vl_free(),
        ),
        ("nvidia/nemotron-nano-9b-v2", nvidia_nemotron_nano_9b_v2()),
        (
            "nvidia/nemotron-nano-9b-v2:free",
            nvidia_nemotron_nano_9b_v2_free(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn nvidia_llama_3_dot_1_nemotron_70b_instruct() -> Model {
    Model {
        id: "nvidia/llama-3.1-nemotron-70b-instruct".into(),
        name: "NVIDIA: Llama 3.1 Nemotron 70B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 1.2,
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
fn nvidia_llama_3_dot_3_nemotron_super_49b_v1_dot_5() -> Model {
    Model {
        id: "nvidia/llama-3.3-nemotron-super-49b-v1.5".into(),
        name: "NVIDIA: Llama 3.3 Nemotron Super 49B V1.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.399_999_999_999_999_97,
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
fn nvidia_nemotron_3_nano_30b_a3b() -> Model {
    Model {
        id: "nvidia/nemotron-3-nano-30b-a3b".into(),
        name: "NVIDIA: Nemotron 3 Nano 30B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.199_999_999_999_999_98,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 228_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn nvidia_nemotron_3_nano_30b_a3b_free() -> Model {
    Model {
        id: "nvidia/nemotron-3-nano-30b-a3b:free".into(),
        name: "NVIDIA: Nemotron 3 Nano 30B A3B (free)".into(),
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
        context_window: 256_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn nvidia_nemotron_3_nano_omni_30b_a3b_reasoning_free() -> Model {
    Model {
        id: "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free".into(),
        name: "NVIDIA: Nemotron 3 Nano Omni (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn nvidia_nemotron_3_super_120b_a12b() -> Model {
    Model {
        id: "nvidia/nemotron-3-super-120b-a12b".into(),
        name: "NVIDIA: Nemotron 3 Super".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn nvidia_nemotron_3_super_120b_a12b_free() -> Model {
    Model {
        id: "nvidia/nemotron-3-super-120b-a12b:free".into(),
        name: "NVIDIA: Nemotron 3 Super (free)".into(),
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
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn nvidia_nemotron_nano_12b_v2_vl_free() -> Model {
    Model {
        id: "nvidia/nemotron-nano-12b-v2-vl:free".into(),
        name: "NVIDIA: Nemotron Nano 12B 2 VL (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn nvidia_nemotron_nano_9b_v2() -> Model {
    Model {
        id: "nvidia/nemotron-nano-9b-v2".into(),
        name: "NVIDIA: Nemotron Nano 9B V2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.04,
            output: 0.16,
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
fn nvidia_nemotron_nano_9b_v2_free() -> Model {
    Model {
        id: "nvidia/nemotron-nano-9b-v2:free".into(),
        name: "NVIDIA: Nemotron Nano 9B V2 (free)".into(),
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
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_118() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-3.5-turbo", openai_gpt_3_dot_5_turbo()),
        ("openai/gpt-3.5-turbo-0613", openai_gpt_3_dot_5_turbo_0613()),
        ("openai/gpt-3.5-turbo-16k", openai_gpt_3_dot_5_turbo_16k()),
        ("openai/gpt-4", openai_gpt_4()),
        ("openai/gpt-4-0314", openai_gpt_4_0314()),
        ("openai/gpt-4-1106-preview", openai_gpt_4_1106_preview()),
        ("openai/gpt-4-turbo", openai_gpt_4_turbo()),
        ("openai/gpt-4-turbo-preview", openai_gpt_4_turbo_preview()),
        ("openai/gpt-4.1", openai_gpt_4_dot_1()),
        ("openai/gpt-4.1-mini", openai_gpt_4_dot_1_mini()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_128() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-4.1-nano", openai_gpt_4_dot_1_nano()),
        ("openai/gpt-4o", openai_gpt_4o()),
        ("openai/gpt-4o-2024-05-13", openai_gpt_4o_2024_05_13()),
        ("openai/gpt-4o-2024-08-06", openai_gpt_4o_2024_08_06()),
        ("openai/gpt-4o-2024-11-20", openai_gpt_4o_2024_11_20()),
        ("openai/gpt-4o-audio-preview", openai_gpt_4o_audio_preview()),
        ("openai/gpt-4o-mini", openai_gpt_4o_mini()),
        (
            "openai/gpt-4o-mini-2024-07-18",
            openai_gpt_4o_mini_2024_07_18(),
        ),
        ("openai/gpt-5", openai_gpt_5()),
        ("openai/gpt-5-codex", openai_gpt_5_codex()),
    ]
}
/// Construct the recorded descriptor for this model.
fn openai_gpt_3_dot_5_turbo() -> Model {
    Model {
        id: "openai/gpt-3.5-turbo".into(),
        name: "OpenAI: GPT-3.5 Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_385.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_3_dot_5_turbo_0613() -> Model {
    Model {
        id: "openai/gpt-3.5-turbo-0613".into(),
        name: "OpenAI: GPT-3.5 Turbo (older v0613)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 4095.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_3_dot_5_turbo_16k() -> Model {
    Model {
        id: "openai/gpt-3.5-turbo-16k".into(),
        name: "OpenAI: GPT-3.5 Turbo 16k".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 3.0,
            output: 4.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_385.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4() -> Model {
    Model {
        id: "openai/gpt-4".into(),
        name: "OpenAI: GPT-4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 30.0,
            output: 60.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8191.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4_0314() -> Model {
    Model {
        id: "openai/gpt-4-0314".into(),
        name: "OpenAI: GPT-4 (older v0314)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 30.0,
            output: 60.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8191.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4_1106_preview() -> Model {
    Model {
        id: "openai/gpt-4-1106-preview".into(),
        name: "OpenAI: GPT-4 Turbo (older v1106)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
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
fn openai_gpt_4_turbo() -> Model {
    Model {
        id: "openai/gpt-4-turbo".into(),
        name: "OpenAI: GPT-4 Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_gpt_4_turbo_preview() -> Model {
    Model {
        id: "openai/gpt-4-turbo-preview".into(),
        name: "OpenAI: GPT-4 Turbo Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
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
        name: "OpenAI: GPT-4.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4_dot_1_mini() -> Model {
    Model {
        id: "openai/gpt-4.1-mini".into(),
        name: "OpenAI: GPT-4.1 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-4.1 Nano".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-4o".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_4o_2024_05_13() -> Model {
    Model {
        id: "openai/gpt-4o-2024-05-13".into(),
        name: "OpenAI: GPT-4o (2024-05-13)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 15.0,
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
fn openai_gpt_4o_2024_08_06() -> Model {
    Model {
        id: "openai/gpt-4o-2024-08-06".into(),
        name: "OpenAI: GPT-4o (2024-08-06)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_gpt_4o_2024_11_20() -> Model {
    Model {
        id: "openai/gpt-4o-2024-11-20".into(),
        name: "OpenAI: GPT-4o (2024-11-20)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_gpt_4o_audio_preview() -> Model {
    Model {
        id: "openai/gpt-4o-audio-preview".into(),
        name: "OpenAI: GPT-4o Audio".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
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
        name: "OpenAI: GPT-4o-mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_gpt_4o_mini_2024_07_18() -> Model {
    Model {
        id: "openai/gpt-4o-mini-2024-07-18".into(),
        name: "OpenAI: GPT-4o-mini (2024-07-18)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_gpt_5_codex() -> Model {
    Model {
        id: "openai/gpt-5-codex".into(),
        name: "OpenAI: GPT-5 Codex".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_138() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-5-mini", openai_gpt_5_mini()),
        ("openai/gpt-5-nano", openai_gpt_5_nano()),
        ("openai/gpt-5-pro", openai_gpt_5_pro()),
        ("openai/gpt-5.1", openai_gpt_5_dot_1()),
        ("openai/gpt-5.1-chat", openai_gpt_5_dot_1_chat()),
        ("openai/gpt-5.1-codex", openai_gpt_5_dot_1_codex()),
        ("openai/gpt-5.1-codex-max", openai_gpt_5_dot_1_codex_max()),
        ("openai/gpt-5.1-codex-mini", openai_gpt_5_dot_1_codex_mini()),
        ("openai/gpt-5.2", openai_gpt_5_dot_2()),
        ("openai/gpt-5.2-chat", openai_gpt_5_dot_2_chat()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_148() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-5.2-codex", openai_gpt_5_dot_2_codex()),
        ("openai/gpt-5.2-pro", openai_gpt_5_dot_2_pro()),
        ("openai/gpt-5.3-chat", openai_gpt_5_dot_3_chat()),
        ("openai/gpt-5.3-codex", openai_gpt_5_dot_3_codex()),
        ("openai/gpt-5.4", openai_gpt_5_dot_4()),
        ("openai/gpt-5.4-mini", openai_gpt_5_dot_4_mini()),
        ("openai/gpt-5.4-nano", openai_gpt_5_dot_4_nano()),
        ("openai/gpt-5.4-pro", openai_gpt_5_dot_4_pro()),
        ("openai/gpt-5.5", openai_gpt_5_dot_5()),
        ("openai/gpt-5.5-pro", openai_gpt_5_dot_5_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn openai_gpt_5_mini() -> Model {
    Model {
        id: "openai/gpt-5-mini".into(),
        name: "OpenAI: GPT-5 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5 Nano".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_pro() -> Model {
    Model {
        id: "openai/gpt-5-pro".into(),
        name: "OpenAI: GPT-5 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_1() -> Model {
    Model {
        id: "openai/gpt-5.1".into(),
        name: "OpenAI: GPT-5.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.13,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_1_chat() -> Model {
    Model {
        id: "openai/gpt-5.1-chat".into(),
        name: "OpenAI: GPT-5.1 Chat".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
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
fn openai_gpt_5_dot_1_codex() -> Model {
    Model {
        id: "openai/gpt-5.1-codex".into(),
        name: "OpenAI: GPT-5.1-Codex".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.1-Codex-Max".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.1-Codex-Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.03,
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
        name: "OpenAI: GPT-5.2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.2 Chat".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.75,
            output: 14.0,
            cache_read: 0.175,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_2_codex() -> Model {
    Model {
        id: "openai/gpt-5.2-codex".into(),
        name: "OpenAI: GPT-5.2-Codex".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_gpt_5_dot_2_pro() -> Model {
    Model {
        id: "openai/gpt-5.2-pro".into(),
        name: "OpenAI: GPT-5.2 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.3 Chat".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
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
        name: "OpenAI: GPT-5.3-Codex".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.4 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.4 Nano".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.4 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: GPT-5.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_5_dot_5_pro() -> Model {
    Model {
        id: "openai/gpt-5.5-pro".into(),
        name: "OpenAI: GPT-5.5 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_158() -> [(&'static str, Model); 10] {
    [
        ("openai/gpt-audio", openai_gpt_audio()),
        ("openai/gpt-audio-mini", openai_gpt_audio_mini()),
        ("openai/gpt-oss-120b", openai_gpt_oss_120b()),
        ("openai/gpt-oss-120b:free", openai_gpt_oss_120b_free()),
        ("openai/gpt-oss-20b", openai_gpt_oss_20b()),
        ("openai/gpt-oss-20b:free", openai_gpt_oss_20b_free()),
        (
            "openai/gpt-oss-safeguard-20b",
            openai_gpt_oss_safeguard_20b(),
        ),
        ("openai/o1", openai_o1()),
        ("openai/o3", openai_o3()),
        ("openai/o3-deep-research", openai_o3_deep_research()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_168() -> [(&'static str, Model); 6] {
    [
        ("openai/o3-mini", openai_o3_mini()),
        ("openai/o3-mini-high", openai_o3_mini_high()),
        ("openai/o3-pro", openai_o3_pro()),
        ("openai/o4-mini", openai_o4_mini()),
        (
            "openai/o4-mini-deep-research",
            openai_o4_mini_deep_research(),
        ),
        ("openai/o4-mini-high", openai_o4_mini_high()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_272() -> [(&'static str, Model); 2] {
    [
        ("~openai/gpt-latest", openai_gpt_latest()),
        ("~openai/gpt-mini-latest", openai_gpt_mini_latest()),
    ]
}
/// Construct the recorded descriptor for this model.
fn openai_gpt_audio() -> Model {
    Model {
        id: "openai/gpt-audio".into(),
        name: "OpenAI: GPT Audio".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_audio_mini() -> Model {
    Model {
        id: "openai/gpt-audio-mini".into(),
        name: "OpenAI: GPT Audio Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_120b() -> Model {
    Model {
        id: "openai/gpt-oss-120b".into(),
        name: "OpenAI: gpt-oss-120b".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.039,
            output: 0.18,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_120b_free() -> Model {
    Model {
        id: "openai/gpt-oss-120b:free".into(),
        name: "OpenAI: gpt-oss-120b (free)".into(),
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
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_20b() -> Model {
    Model {
        id: "openai/gpt-oss-20b".into(),
        name: "OpenAI: gpt-oss-20b".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.03,
            output: 0.14,
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
fn openai_gpt_oss_20b_free() -> Model {
    Model {
        id: "openai/gpt-oss-20b:free".into(),
        name: "OpenAI: gpt-oss-20b (free)".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_oss_safeguard_20b() -> Model {
    Model {
        id: "openai/gpt-oss-safeguard-20b".into(),
        name: "OpenAI: gpt-oss-safeguard-20b".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: o1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: o3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: o3 Deep Research".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: o3 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_o3_mini_high() -> Model {
    Model {
        id: "openai/o3-mini-high".into(),
        name: "OpenAI: o3 Mini High".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: o3 Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        name: "OpenAI: o4 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

/// Construct the recorded descriptor for this model.
fn openai_o4_mini_deep_research() -> Model {
    Model {
        id: "openai/o4-mini-deep-research".into(),
        name: "OpenAI: o4 Mini Deep Research".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn openai_o4_mini_high() -> Model {
    Model {
        id: "openai/o4-mini-high".into(),
        name: "OpenAI: o4 Mini High".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

/// Construct the recorded descriptor for this model.
fn openai_gpt_latest() -> Model {
    Model {
        id: "~openai/gpt-latest".into(),
        name: "OpenAI GPT Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 5.0,
            output: 30.0,
            cache_read: 0.5,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openai_gpt_mini_latest() -> Model {
    Model {
        id: "~openai/gpt-mini-latest".into(),
        name: "OpenAI GPT Mini Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_174() -> [(&'static str, Model); 3] {
    [
        ("openrouter/auto", openrouter_auto()),
        ("openrouter/free", openrouter_free()),
        ("openrouter/owl-alpha", openrouter_owl_alpha()),
    ]
}
/// Construct the recorded descriptor for this model.
fn openrouter_auto() -> Model {
    Model {
        id: "openrouter/auto".into(),
        name: "Auto Router".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: -1_000_000.0,
            output: -1_000_000.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openrouter_free() -> Model {
    Model {
        id: "openrouter/free".into(),
        name: "Free Models Router".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn openrouter_owl_alpha() -> Model {
    Model {
        id: "openrouter/owl-alpha".into(),
        name: "Owl Alpha".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_048_756.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_177() -> [(&'static str, Model); 2] {
    [
        ("poolside/laguna-m.1:free", poolside_laguna_m_dot_1_free()),
        ("poolside/laguna-xs.2:free", poolside_laguna_xs_dot_2_free()),
    ]
}
/// Construct the recorded descriptor for this model.
fn poolside_laguna_m_dot_1_free() -> Model {
    Model {
        id: "poolside/laguna-m.1:free".into(),
        name: "Poolside: Laguna M.1 (free)".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn poolside_laguna_xs_dot_2_free() -> Model {
    Model {
        id: "poolside/laguna-xs.2:free".into(),
        name: "Poolside: Laguna XS.2 (free)".into(),
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
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_179() -> [(&'static str, Model); 1] {
    [("prime-intellect/intellect-3", prime_intellect_intellect_3())]
}
/// Construct the recorded descriptor for this model.
fn prime_intellect_intellect_3() -> Model {
    Model {
        id: "prime-intellect/intellect-3".into(),
        name: "Prime Intellect: INTELLECT-3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.1,
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
pub(super) fn models_180() -> [(&'static str, Model); 10] {
    [
        (
            "qwen/qwen-2.5-72b-instruct",
            qwen_qwen_2_dot_5_72b_instruct(),
        ),
        ("qwen/qwen-2.5-7b-instruct", qwen_qwen_2_dot_5_7b_instruct()),
        ("qwen/qwen-max", qwen_qwen_max()),
        ("qwen/qwen-plus", qwen_qwen_plus()),
        ("qwen/qwen-plus-2025-07-28", qwen_qwen_plus_2025_07_28()),
        (
            "qwen/qwen-plus-2025-07-28:thinking",
            qwen_qwen_plus_2025_07_28_thinking(),
        ),
        ("qwen/qwen-turbo", qwen_qwen_turbo()),
        ("qwen/qwen-vl-max", qwen_qwen_vl_max()),
        ("qwen/qwen3-14b", qwen_qwen3_14b()),
        ("qwen/qwen3-235b-a22b", qwen_qwen3_235b_a22b()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_190() -> [(&'static str, Model); 9] {
    [
        ("qwen/qwen3-235b-a22b-2507", qwen_qwen3_235b_a22b_2507()),
        (
            "qwen/qwen3-235b-a22b-thinking-2507",
            qwen_qwen3_235b_a22b_thinking_2507(),
        ),
        ("qwen/qwen3-30b-a3b", qwen_qwen3_30b_a3b()),
        (
            "qwen/qwen3-30b-a3b-instruct-2507",
            qwen_qwen3_30b_a3b_instruct_2507(),
        ),
        (
            "qwen/qwen3-30b-a3b-thinking-2507",
            qwen_qwen3_30b_a3b_thinking_2507(),
        ),
        ("qwen/qwen3-32b", qwen_qwen3_32b()),
        ("qwen/qwen3-8b", qwen_qwen3_8b()),
        ("qwen/qwen3-coder", qwen_qwen3_coder()),
        (
            "qwen/qwen3-coder-30b-a3b-instruct",
            qwen_qwen3_coder_30b_a3b_instruct(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn qwen_qwen_2_dot_5_72b_instruct() -> Model {
    Model {
        id: "qwen/qwen-2.5-72b-instruct".into(),
        name: "Qwen2.5 72B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.36,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen_2_dot_5_7b_instruct() -> Model {
    Model {
        id: "qwen/qwen-2.5-7b-instruct".into(),
        name: "Qwen: Qwen2.5 7B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.04,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen_max() -> Model {
    Model {
        id: "qwen/qwen-max".into(),
        name: "Qwen: Qwen-Max ".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.04,
            output: 4.16,
            cache_read: 0.208_000_000_000_000_02,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen_plus() -> Model {
    Model {
        id: "qwen/qwen-plus".into(),
        name: "Qwen: Qwen-Plus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.26,
            output: 0.78,
            cache_read: 0.052_000_000_000_000_005,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen_plus_2025_07_28() -> Model {
    Model {
        id: "qwen/qwen-plus-2025-07-28".into(),
        name: "Qwen: Qwen Plus 0728".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.26,
            output: 0.78,
            cache_read: 0.0,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen_plus_2025_07_28_thinking() -> Model {
    Model {
        id: "qwen/qwen-plus-2025-07-28:thinking".into(),
        name: "Qwen: Qwen Plus 0728 (thinking)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.26,
            output: 0.78,
            cache_read: 0.0,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen_turbo() -> Model {
    Model {
        id: "qwen/qwen-turbo".into(),
        name: "Qwen: Qwen-Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0325,
            output: 0.13,
            cache_read: 0.006_500_000_000_000_001,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen_vl_max() -> Model {
    Model {
        id: "qwen/qwen-vl-max".into(),
        name: "Qwen: Qwen VL Max".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.52,
            output: 2.08,
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
fn qwen_qwen3_14b() -> Model {
    Model {
        id: "qwen/qwen3-14b".into(),
        name: "Qwen: Qwen3 14B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 40_960.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_235b_a22b() -> Model {
    Model {
        id: "qwen/qwen3-235b-a22b".into(),
        name: "Qwen: Qwen3 235B A22B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.454_999_999_999_999_96,
            output: 1.819_999_999_999_999_8,
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
fn qwen_qwen3_235b_a22b_2507() -> Model {
    Model {
        id: "qwen/qwen3-235b-a22b-2507".into(),
        name: "Qwen: Qwen3 235B A22B Instruct 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.071,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_235b_a22b_thinking_2507() -> Model {
    Model {
        id: "qwen/qwen3-235b-a22b-thinking-2507".into(),
        name: "Qwen: Qwen3 235B A22B Thinking 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.149_500_000_000_000_02,
            output: 1.495,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_30b_a3b() -> Model {
    Model {
        id: "qwen/qwen3-30b-a3b".into(),
        name: "Qwen: Qwen3 30B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.449_999_999_999_999_96,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 20_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_30b_a3b_instruct_2507() -> Model {
    Model {
        id: "qwen/qwen3-30b-a3b-instruct-2507".into(),
        name: "Qwen: Qwen3 30B A3B Instruct 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_30b_a3b_thinking_2507() -> Model {
    Model {
        id: "qwen/qwen3-30b-a3b-thinking-2507".into(),
        name: "Qwen: Qwen3 30B A3B Thinking 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_32b() -> Model {
    Model {
        id: "qwen/qwen3-32b".into(),
        name: "Qwen: Qwen3 32B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.24,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 40_960.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_8b() -> Model {
    Model {
        id: "qwen/qwen3-8b".into(),
        name: "Qwen: Qwen3 8B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 40_960.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder() -> Model {
    Model {
        id: "qwen/qwen3-coder".into(),
        name: "Qwen: Qwen3 Coder 480B A35B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 1.799_999_999_999_999_8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder_30b_a3b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-coder-30b-a3b-instruct".into(),
        name: "Qwen: Qwen3 Coder 30B A3B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.27,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 160_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_199() -> [(&'static str, Model); 10] {
    [
        ("qwen/qwen3-coder-flash", qwen_qwen3_coder_flash()),
        ("qwen/qwen3-coder-next", qwen_qwen3_coder_next()),
        ("qwen/qwen3-coder-plus", qwen_qwen3_coder_plus()),
        ("qwen/qwen3-coder:free", qwen_qwen3_coder_free()),
        ("qwen/qwen3-max", qwen_qwen3_max()),
        ("qwen/qwen3-max-thinking", qwen_qwen3_max_thinking()),
        (
            "qwen/qwen3-next-80b-a3b-instruct",
            qwen_qwen3_next_80b_a3b_instruct(),
        ),
        (
            "qwen/qwen3-next-80b-a3b-instruct:free",
            qwen_qwen3_next_80b_a3b_instruct_free(),
        ),
        (
            "qwen/qwen3-next-80b-a3b-thinking",
            qwen_qwen3_next_80b_a3b_thinking(),
        ),
        (
            "qwen/qwen3-vl-235b-a22b-instruct",
            qwen_qwen3_vl_235b_a22b_instruct(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_209() -> [(&'static str, Model); 9] {
    [
        (
            "qwen/qwen3-vl-235b-a22b-thinking",
            qwen_qwen3_vl_235b_a22b_thinking(),
        ),
        (
            "qwen/qwen3-vl-30b-a3b-instruct",
            qwen_qwen3_vl_30b_a3b_instruct(),
        ),
        (
            "qwen/qwen3-vl-30b-a3b-thinking",
            qwen_qwen3_vl_30b_a3b_thinking(),
        ),
        ("qwen/qwen3-vl-32b-instruct", qwen_qwen3_vl_32b_instruct()),
        ("qwen/qwen3-vl-8b-instruct", qwen_qwen3_vl_8b_instruct()),
        ("qwen/qwen3-vl-8b-thinking", qwen_qwen3_vl_8b_thinking()),
        ("qwen/qwen3.5-122b-a10b", qwen_qwen3_dot_5_122b_a10b()),
        ("qwen/qwen3.5-27b", qwen_qwen3_dot_5_27b()),
        ("qwen/qwen3.5-35b-a3b", qwen_qwen3_dot_5_35b_a3b()),
    ]
}
/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder_flash() -> Model {
    Model {
        id: "qwen/qwen3-coder-flash".into(),
        name: "Qwen: Qwen3 Coder Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.195,
            output: 0.975,
            cache_read: 0.039,
            cache_write: 0.243_75,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder_next() -> Model {
    Model {
        id: "qwen/qwen3-coder-next".into(),
        name: "Qwen: Qwen3 Coder Next".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.12,
            output: 0.799_999_999_999_999_9,
            cache_read: 0.07,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder_plus() -> Model {
    Model {
        id: "qwen/qwen3-coder-plus".into(),
        name: "Qwen: Qwen3 Coder Plus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.65,
            output: 3.25,
            cache_read: 0.13,
            cache_write: 0.8125,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_coder_free() -> Model {
    Model {
        id: "qwen/qwen3-coder:free".into(),
        name: "Qwen: Qwen3 Coder 480B A35B (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_max() -> Model {
    Model {
        id: "qwen/qwen3-max".into(),
        name: "Qwen: Qwen3 Max".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.78,
            output: 3.9,
            cache_read: 0.156,
            cache_write: 0.975,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_max_thinking() -> Model {
    Model {
        id: "qwen/qwen3-max-thinking".into(),
        name: "Qwen: Qwen3 Max Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.78,
            output: 3.9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_next_80b_a3b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-next-80b-a3b-instruct".into(),
        name: "Qwen: Qwen3 Next 80B A3B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 1.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_next_80b_a3b_instruct_free() -> Model {
    Model {
        id: "qwen/qwen3-next-80b-a3b-instruct:free".into(),
        name: "Qwen: Qwen3 Next 80B A3B Instruct (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_next_80b_a3b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-next-80b-a3b-thinking".into(),
        name: "Qwen: Qwen3 Next 80B A3B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0975,
            output: 0.78,
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
fn qwen_qwen3_vl_235b_a22b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-235b-a22b-instruct".into(),
        name: "Qwen: Qwen3 VL 235B A22B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.88,
            cache_read: 0.11,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_vl_235b_a22b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-vl-235b-a22b-thinking".into(),
        name: "Qwen: Qwen3 VL 235B A22B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.26,
            output: 2.6,
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
fn qwen_qwen3_vl_30b_a3b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-30b-a3b-instruct".into(),
        name: "Qwen: Qwen3 VL 30B A3B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 0.52,
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
fn qwen_qwen3_vl_30b_a3b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-vl-30b-a3b-thinking".into(),
        name: "Qwen: Qwen3 VL 30B A3B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.13,
            output: 1.56,
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
fn qwen_qwen3_vl_32b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-32b-instruct".into(),
        name: "Qwen: Qwen3 VL 32B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.104_000_000_000_000_01,
            output: 0.416_000_000_000_000_04,
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
fn qwen_qwen3_vl_8b_instruct() -> Model {
    Model {
        id: "qwen/qwen3-vl-8b-instruct".into(),
        name: "Qwen: Qwen3 VL 8B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.08,
            output: 0.5,
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
fn qwen_qwen3_vl_8b_thinking() -> Model {
    Model {
        id: "qwen/qwen3-vl-8b-thinking".into(),
        name: "Qwen: Qwen3 VL 8B Thinking".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.117,
            output: 1.365,
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
fn qwen_qwen3_dot_5_122b_a10b() -> Model {
    Model {
        id: "qwen/qwen3.5-122b-a10b".into(),
        name: "Qwen: Qwen3.5-122B-A10B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.26,
            output: 2.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_27b() -> Model {
    Model {
        id: "qwen/qwen3.5-27b".into(),
        name: "Qwen: Qwen3.5-27B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.195,
            output: 1.56,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_35b_a3b() -> Model {
    Model {
        id: "qwen/qwen3.5-35b-a3b".into(),
        name: "Qwen: Qwen3.5-35B-A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 1.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_218() -> [(&'static str, Model); 10] {
    [
        ("qwen/qwen3.5-397b-a17b", qwen_qwen3_dot_5_397b_a17b()),
        ("qwen/qwen3.5-9b", qwen_qwen3_dot_5_9b()),
        ("qwen/qwen3.5-flash-02-23", qwen_qwen3_dot_5_flash_02_23()),
        ("qwen/qwen3.5-plus-02-15", qwen_qwen3_dot_5_plus_02_15()),
        (
            "qwen/qwen3.5-plus-20260420",
            qwen_qwen3_dot_5_plus_20260420(),
        ),
        ("qwen/qwen3.6-27b", qwen_qwen3_dot_6_27b()),
        ("qwen/qwen3.6-35b-a3b", qwen_qwen3_dot_6_35b_a3b()),
        ("qwen/qwen3.6-flash", qwen_qwen3_dot_6_flash()),
        ("qwen/qwen3.6-max-preview", qwen_qwen3_dot_6_max_preview()),
        ("qwen/qwen3.6-plus", qwen_qwen3_dot_6_plus()),
    ]
}
/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_397b_a17b() -> Model {
    Model {
        id: "qwen/qwen3.5-397b-a17b".into(),
        name: "Qwen: Qwen3.5 397B A17B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.39,
            output: 2.34,
            cache_read: 0.195,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_9b() -> Model {
    Model {
        id: "qwen/qwen3.5-9b".into(),
        name: "Qwen: Qwen3.5-9B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_flash_02_23() -> Model {
    Model {
        id: "qwen/qwen3.5-flash-02-23".into(),
        name: "Qwen: Qwen3.5-Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.065,
            output: 0.26,
            cache_read: 0.0,
            cache_write: 0.081_25,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_plus_02_15() -> Model {
    Model {
        id: "qwen/qwen3.5-plus-02-15".into(),
        name: "Qwen: Qwen3.5 Plus 2026-02-15".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.26,
            output: 1.56,
            cache_read: 0.0,
            cache_write: 0.325,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_5_plus_20260420() -> Model {
    Model {
        id: "qwen/qwen3.5-plus-20260420".into(),
        name: "Qwen: Qwen3.5 Plus 2026-04-20".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_6_27b() -> Model {
    Model {
        id: "qwen/qwen3.6-27b".into(),
        name: "Qwen: Qwen3.6 27B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.32,
            output: 3.199_999_999_999_999_7,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 81_920.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_6_35b_a3b() -> Model {
    Model {
        id: "qwen/qwen3.6-35b-a3b".into(),
        name: "Qwen: Qwen3.6 35B A3B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 1.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_6_flash() -> Model {
    Model {
        id: "qwen/qwen3.6-flash".into(),
        name: "Qwen: Qwen3.6 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.3125,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_6_max_preview() -> Model {
    Model {
        id: "qwen/qwen3.6-max-preview".into(),
        name: "Qwen: Qwen3.6 Max Preview".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.04,
            output: 6.24,
            cache_read: 0.0,
            cache_write: 1.3,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_qwen3_dot_6_plus() -> Model {
    Model {
        id: "qwen/qwen3.6-plus".into(),
        name: "Qwen: Qwen3.6 Plus".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.325,
            output: 1.95,
            cache_read: 0.0,
            cache_write: 0.406_25,
        },
        context_window: 1_000_000.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_228() -> [(&'static str, Model); 1] {
    [("rekaai/reka-edge", rekaai_reka_edge())]
}
/// Construct the recorded descriptor for this model.
fn rekaai_reka_edge() -> Model {
    Model {
        id: "rekaai/reka-edge".into(),
        name: "Reka Edge".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_384.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_229() -> [(&'static str, Model); 1] {
    [("relace/relace-search", relace_relace_search())]
}
/// Construct the recorded descriptor for this model.
fn relace_relace_search() -> Model {
    Model {
        id: "relace/relace-search".into(),
        name: "Relace: Relace Search".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_230() -> [(&'static str, Model); 2] {
    [
        ("sao10k/l3-euryale-70b", sao10k_l3_euryale_70b()),
        ("sao10k/l3.1-euryale-70b", sao10k_l3_dot_1_euryale_70b()),
    ]
}
/// Construct the recorded descriptor for this model.
fn sao10k_l3_euryale_70b() -> Model {
    Model {
        id: "sao10k/l3-euryale-70b".into(),
        name: "Sao10k: Llama 3 Euryale 70B v2.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.48,
            output: 1.48,
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
fn sao10k_l3_dot_1_euryale_70b() -> Model {
    Model {
        id: "sao10k/l3.1-euryale-70b".into(),
        name: "Sao10K: Llama 3.1 Euryale 70B v2.2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.85,
            output: 0.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_232() -> [(&'static str, Model); 1] {
    [("stepfun/step-3.5-flash", stepfun_step_3_dot_5_flash())]
}
/// Construct the recorded descriptor for this model.
fn stepfun_step_3_dot_5_flash() -> Model {
    Model {
        id: "stepfun/step-3.5-flash".into(),
        name: "StepFun: Step 3.5 Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_233() -> [(&'static str, Model); 1] {
    [("tencent/hy3-preview:free", tencent_hy3_preview_free())]
}
/// Construct the recorded descriptor for this model.
fn tencent_hy3_preview_free() -> Model {
    Model {
        id: "tencent/hy3-preview:free".into(),
        name: "Tencent: Hy3 preview (free)".into(),
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
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_234() -> [(&'static str, Model); 2] {
    [
        ("thedrummer/rocinante-12b", thedrummer_rocinante_12b()),
        ("thedrummer/unslopnemo-12b", thedrummer_unslopnemo_12b()),
    ]
}
/// Construct the recorded descriptor for this model.
fn thedrummer_rocinante_12b() -> Model {
    Model {
        id: "thedrummer/rocinante-12b".into(),
        name: "TheDrummer: Rocinante 12B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.169_999_999_999_999_98,
            output: 0.43,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn thedrummer_unslopnemo_12b() -> Model {
    Model {
        id: "thedrummer/unslopnemo-12b".into(),
        name: "TheDrummer: UnslopNemo 12B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 0.399_999_999_999_999_97,
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
pub(super) fn models_236() -> [(&'static str, Model); 1] {
    [(
        "tngtech/deepseek-r1t2-chimera",
        tngtech_deepseek_r1t2_chimera(),
    )]
}
/// Construct the recorded descriptor for this model.
fn tngtech_deepseek_r1t2_chimera() -> Model {
    Model {
        id: "tngtech/deepseek-r1t2-chimera".into(),
        name: "TNG: DeepSeek R1T2 Chimera".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.1,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 163_840.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_237() -> [(&'static str, Model); 1] {
    [("upstage/solar-pro-3", upstage_solar_pro_3())]
}
/// Construct the recorded descriptor for this model.
fn upstage_solar_pro_3() -> Model {
    Model {
        id: "upstage/solar-pro-3".into(),
        name: "Upstage: Solar Pro 3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_238() -> [(&'static str, Model); 10] {
    [
        ("x-ai/grok-3", x_ai_grok_3()),
        ("x-ai/grok-3-beta", x_ai_grok_3_beta()),
        ("x-ai/grok-3-mini", x_ai_grok_3_mini()),
        ("x-ai/grok-3-mini-beta", x_ai_grok_3_mini_beta()),
        ("x-ai/grok-4", x_ai_grok_4()),
        ("x-ai/grok-4-fast", x_ai_grok_4_fast()),
        ("x-ai/grok-4.1-fast", x_ai_grok_4_dot_1_fast()),
        ("x-ai/grok-4.20", x_ai_grok_4_dot_20()),
        ("x-ai/grok-4.3", x_ai_grok_4_dot_3()),
        ("x-ai/grok-code-fast-1", x_ai_grok_code_fast_1()),
    ]
}
/// Construct the recorded descriptor for this model.
fn x_ai_grok_3() -> Model {
    Model {
        id: "x-ai/grok-3".into(),
        name: "xAI: Grok 3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn x_ai_grok_3_beta() -> Model {
    Model {
        id: "x-ai/grok-3-beta".into(),
        name: "xAI: Grok 3 Beta".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn x_ai_grok_3_mini() -> Model {
    Model {
        id: "x-ai/grok-3-mini".into(),
        name: "xAI: Grok 3 Mini".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn x_ai_grok_3_mini_beta() -> Model {
    Model {
        id: "x-ai/grok-3-mini-beta".into(),
        name: "xAI: Grok 3 Mini Beta".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.5,
            cache_read: 0.075,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn x_ai_grok_4() -> Model {
    Model {
        id: "x-ai/grok-4".into(),
        name: "xAI: Grok 4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn x_ai_grok_4_fast() -> Model {
    Model {
        id: "x-ai/grok-4-fast".into(),
        name: "xAI: Grok 4 Fast".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn x_ai_grok_4_dot_1_fast() -> Model {
    Model {
        id: "x-ai/grok-4.1-fast".into(),
        name: "xAI: Grok 4.1 Fast".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
fn x_ai_grok_4_dot_20() -> Model {
    Model {
        id: "x-ai/grok-4.20".into(),
        name: "xAI: Grok 4.20".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 2.5,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn x_ai_grok_4_dot_3() -> Model {
    Model {
        id: "x-ai/grok-4.3".into(),
        name: "xAI: Grok 4.3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn x_ai_grok_code_fast_1() -> Model {
    Model {
        id: "x-ai/grok-code-fast-1".into(),
        name: "xAI: Grok Code Fast 1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
        max_tokens: 10_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_248() -> [(&'static str, Model); 5] {
    [
        ("xiaomi/mimo-v2-flash", xiaomi_mimo_v2_flash()),
        ("xiaomi/mimo-v2-omni", xiaomi_mimo_v2_omni()),
        ("xiaomi/mimo-v2-pro", xiaomi_mimo_v2_pro()),
        ("xiaomi/mimo-v2.5", xiaomi_mimo_v2_dot_5()),
        ("xiaomi/mimo-v2.5-pro", xiaomi_mimo_v2_dot_5_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_flash() -> Model {
    Model {
        id: "xiaomi/mimo-v2-flash".into(),
        name: "Xiaomi: MiMo-V2-Flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.09,
            output: 0.29,
            cache_read: 0.045,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_omni() -> Model {
    Model {
        id: "xiaomi/mimo-v2-omni".into(),
        name: "Xiaomi: MiMo-V2-Omni".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2-pro".into(),
        name: "Xiaomi: MiMo-V2-Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_dot_5() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5".into(),
        name: "Xiaomi: MiMo-V2.5".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
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

/// Construct the recorded descriptor for this model.
fn xiaomi_mimo_v2_dot_5_pro() -> Model {
    Model {
        id: "xiaomi/mimo-v2.5-pro".into(),
        name: "Xiaomi: MiMo-V2.5-Pro".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_253() -> [(&'static str, Model); 10] {
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
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_263() -> [(&'static str, Model); 3] {
    [
        ("z-ai/glm-5-turbo", z_ai_glm_5_turbo()),
        ("z-ai/glm-5.1", z_ai_glm_5_dot_1()),
        ("z-ai/glm-5v-turbo", z_ai_glm_5v_turbo()),
    ]
}
/// Construct the recorded descriptor for this model.
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
fn z_ai_glm_5_turbo() -> Model {
    Model {
        id: "z-ai/glm-5-turbo".into(),
        name: "Z.ai: GLM 5 Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn z_ai_glm_5_dot_1() -> Model {
    Model {
        id: "z-ai/glm-5.1".into(),
        name: "Z.ai: GLM 5.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.049_999_999_999_999_8,
            output: 3.5,
            cache_read: 0.524_999_999_999_999_9,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 65_535.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn z_ai_glm_5v_turbo() -> Model {
    Model {
        id: "z-ai/glm-5v-turbo".into(),
        name: "Z.ai: GLM 5V Turbo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.2,
            output: 4.0,
            cache_read: 0.24,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
