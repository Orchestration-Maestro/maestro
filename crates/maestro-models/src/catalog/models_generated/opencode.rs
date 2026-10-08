//! Generated model descriptors for opencode.

use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_1());
    models.extend(models_9());
    models.extend(models_11());
    models.extend(models_13());
    models.extend(models_23());
    models.extend(models_29());
    models.extend(models_30());
    models.extend(models_32());
    models.extend(models_35());
    models.extend(models_36());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [("big-pickle", big_pickle())]
}
/// Construct the recorded descriptor for this model.
fn big_pickle() -> Model {
    Model {
        id: "big-pickle".into(),
        name: "Big Pickle".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_1() -> [(&'static str, Model); 8] {
    [
        ("claude-haiku-4-5", claude_haiku_4_5()),
        ("claude-opus-4-1", claude_opus_4_1()),
        ("claude-opus-4-5", claude_opus_4_5()),
        ("claude-opus-4-6", claude_opus_4_6()),
        ("claude-opus-4-7", claude_opus_4_7()),
        ("claude-sonnet-4", claude_sonnet_4()),
        ("claude-sonnet-4-5", claude_sonnet_4_5()),
        ("claude-sonnet-4-6", claude_sonnet_4_6()),
    ]
}
/// Construct the recorded descriptor for this model.
fn claude_haiku_4_5() -> Model {
    Model {
        id: "claude-haiku-4-5".into(),
        name: "Claude Haiku 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.0,
            output: 5.0,
            cache_read: 0.1,
            cache_write: 1.25,
        },
        context_window: 200_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn claude_opus_4_1() -> Model {
    Model {
        id: "claude-opus-4-1".into(),
        name: "Claude Opus 4.1".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
fn claude_opus_4_5() -> Model {
    Model {
        id: "claude-opus-4-5".into(),
        name: "Claude Opus 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
fn claude_opus_4_6() -> Model {
    Model {
        id: "claude-opus-4-6".into(),
        name: "Claude Opus 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
fn claude_opus_4_7() -> Model {
    Model {
        id: "claude-opus-4-7".into(),
        name: "Claude Opus 4.7".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
fn claude_sonnet_4() -> Model {
    Model {
        id: "claude-sonnet-4".into(),
        name: "Claude Sonnet 4".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
fn claude_sonnet_4_5() -> Model {
    Model {
        id: "claude-sonnet-4-5".into(),
        name: "Claude Sonnet 4.5".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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
fn claude_sonnet_4_6() -> Model {
    Model {
        id: "claude-sonnet-4-6".into(),
        name: "Claude Sonnet 4.6".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_9() -> [(&'static str, Model); 2] {
    [
        ("gemini-3-flash", gemini_3_flash()),
        ("gemini-3.1-pro", gemini_3_dot_1_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn gemini_3_flash() -> Model {
    Model {
        id: "gemini-3-flash".into(),
        name: "Gemini 3 Flash".into(),
        api: "google-generative-ai".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gemini_3_dot_1_pro() -> Model {
    Model {
        id: "gemini-3.1-pro".into(),
        name: "Gemini 3.1 Pro Preview".into(),
        api: "google-generative-ai".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Minimal, None),
                (ModelThinkingLevel::Low, Some("LOW".into())),
                (ModelThinkingLevel::Medium, None),
                (ModelThinkingLevel::High, Some("HIGH".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 12.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_11() -> [(&'static str, Model); 2] {
    [("glm-5", glm_5()), ("glm-5.1", glm_5_dot_1())]
}
/// Construct the recorded descriptor for this model.
fn glm_5() -> Model {
    Model {
        id: "glm-5".into(),
        name: "GLM-5".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.2,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn glm_5_dot_1() -> Model {
    Model {
        id: "glm-5.1".into(),
        name: "GLM-5.1".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_13() -> [(&'static str, Model); 10] {
    [
        ("gpt-5", gpt_5()),
        ("gpt-5-codex", gpt_5_codex()),
        ("gpt-5-nano", gpt_5_nano()),
        ("gpt-5.1", gpt_5_dot_1()),
        ("gpt-5.1-codex", gpt_5_dot_1_codex()),
        ("gpt-5.1-codex-max", gpt_5_dot_1_codex_max()),
        ("gpt-5.1-codex-mini", gpt_5_dot_1_codex_mini()),
        ("gpt-5.2", gpt_5_dot_2()),
        ("gpt-5.2-codex", gpt_5_dot_2_codex()),
        ("gpt-5.3-codex", gpt_5_dot_3_codex()),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_23() -> [(&'static str, Model); 6] {
    [
        ("gpt-5.4", gpt_5_dot_4()),
        ("gpt-5.4-mini", gpt_5_dot_4_mini()),
        ("gpt-5.4-nano", gpt_5_dot_4_nano()),
        ("gpt-5.4-pro", gpt_5_dot_4_pro()),
        ("gpt-5.5", gpt_5_dot_5()),
        ("gpt-5.5-pro", gpt_5_dot_5_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn gpt_5() -> Model {
    Model {
        id: "gpt-5".into(),
        name: "GPT-5".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.07,
            output: 8.5,
            cache_read: 0.107,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_codex() -> Model {
    Model {
        id: "gpt-5-codex".into(),
        name: "GPT-5 Codex".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.07,
            output: 8.5,
            cache_read: 0.107,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_nano() -> Model {
    Model {
        id: "gpt-5-nano".into(),
        name: "GPT-5 Nano".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
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
fn gpt_5_dot_1() -> Model {
    Model {
        id: "gpt-5.1".into(),
        name: "GPT-5.1".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.07,
            output: 8.5,
            cache_read: 0.107,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_1_codex() -> Model {
    Model {
        id: "gpt-5.1-codex".into(),
        name: "GPT-5.1 Codex".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.07,
            output: 8.5,
            cache_read: 0.107,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_1_codex_max() -> Model {
    Model {
        id: "gpt-5.1-codex-max".into(),
        name: "GPT-5.1 Codex Max".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
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
fn gpt_5_dot_1_codex_mini() -> Model {
    Model {
        id: "gpt-5.1-codex-mini".into(),
        name: "GPT-5.1 Codex Mini".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 400_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_2() -> Model {
    Model {
        id: "gpt-5.2".into(),
        name: "GPT-5.2".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
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
fn gpt_5_dot_2_codex() -> Model {
    Model {
        id: "gpt-5.2-codex".into(),
        name: "GPT-5.2 Codex".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
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
fn gpt_5_dot_3_codex() -> Model {
    Model {
        id: "gpt-5.3-codex".into(),
        name: "GPT-5.3 Codex".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
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
fn gpt_5_dot_4() -> Model {
    Model {
        id: "gpt-5.4".into(),
        name: "GPT-5.4".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 15.0,
            cache_read: 0.25,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_4_mini() -> Model {
    Model {
        id: "gpt-5.4-mini".into(),
        name: "GPT-5.4 Mini".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
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
fn gpt_5_dot_4_nano() -> Model {
    Model {
        id: "gpt-5.4-nano".into(),
        name: "GPT-5.4 Nano".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
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
fn gpt_5_dot_4_pro() -> Model {
    Model {
        id: "gpt-5.4-pro".into(),
        name: "GPT-5.4 Pro".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 30.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_5() -> Model {
    Model {
        id: "gpt-5.5".into(),
        name: "GPT-5.5".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
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
fn gpt_5_dot_5_pro() -> Model {
    Model {
        id: "gpt-5.5-pro".into(),
        name: "GPT-5.5 Pro".into(),
        api: "openai-responses".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Off, None),
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 30.0,
            output: 180.0,
            cache_read: 30.0,
            cache_write: 0.0,
        },
        context_window: 1_050_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_29() -> [(&'static str, Model); 1] {
    [("hy3-preview-free", hy3_preview_free())]
}
/// Construct the recorded descriptor for this model.
fn hy3_preview_free() -> Model {
    Model {
        id: "hy3-preview-free".into(),
        name: "Hy3 preview Free".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_30() -> [(&'static str, Model); 2] {
    [
        ("kimi-k2.5", kimi_k2_dot_5()),
        ("kimi-k2.6", kimi_k2_dot_6()),
    ]
}
/// Construct the recorded descriptor for this model.
fn kimi_k2_dot_5() -> Model {
    Model {
        id: "kimi-k2.5".into(),
        name: "Kimi K2.5".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.0,
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
fn kimi_k2_dot_6() -> Model {
    Model {
        id: "kimi-k2.6".into(),
        name: "Kimi K2.6".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_32() -> [(&'static str, Model); 3] {
    [
        ("minimax-m2.5", minimax_m2_dot_5()),
        ("minimax-m2.5-free", minimax_m2_dot_5_free()),
        ("minimax-m2.7", minimax_m2_dot_7()),
    ]
}
/// Construct the recorded descriptor for this model.
fn minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_m2_dot_5_free() -> Model {
    Model {
        id: "minimax-m2.5-free".into(),
        name: "MiniMax M2.5 Free".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_m2_dot_7() -> Model {
    Model {
        id: "minimax-m2.7".into(),
        name: "MiniMax M2.7".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
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
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_35() -> [(&'static str, Model); 1] {
    [("nemotron-3-super-free", nemotron_3_super_free())]
}
/// Construct the recorded descriptor for this model.
fn nemotron_3_super_free() -> Model {
    Model {
        id: "nemotron-3-super-free".into(),
        name: "Nemotron 3 Super Free".into(),
        api: "openai-completions".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_36() -> [(&'static str, Model); 2] {
    [
        ("qwen3.5-plus", qwen3_dot_5_plus()),
        ("qwen3.6-plus", qwen3_dot_6_plus()),
    ]
}
/// Construct the recorded descriptor for this model.
fn qwen3_dot_5_plus() -> Model {
    Model {
        id: "qwen3.5-plus".into(),
        name: "Qwen3.5 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 1.2,
            cache_read: 0.02,
            cache_write: 0.25,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen3_dot_6_plus() -> Model {
    Model {
        id: "qwen3.6-plus".into(),
        name: "Qwen3.6 Plus".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 3.0,
            cache_read: 0.05,
            cache_write: 0.625,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}
