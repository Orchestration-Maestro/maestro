//! Generated model descriptors for opencode-go.

use crate::{
    Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel, OpenAICompletionsCompat,
    ThinkingFormat,
};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_2());
    models.extend(models_4());
    models.extend(models_6());
    models.extend(models_10());
    models.extend(models_12());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 2] {
    [
        ("deepseek-v4-flash", deepseek_v4_flash()),
        ("deepseek-v4-pro", deepseek_v4_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn deepseek_v4_flash() -> Model {
    Model {
        id: "deepseek-v4-flash".into(),
        name: "DeepSeek V4 Flash".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
        context_window: 1_000_000.0,
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
fn deepseek_v4_pro() -> Model {
    Model {
        id: "deepseek-v4-pro".into(),
        name: "DeepSeek V4 Pro".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
            input: 1.74,
            output: 3.48,
            cache_read: 0.0145,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_2() -> [(&'static str, Model); 2] {
    [("glm-5", glm_5()), ("glm-5.1", glm_5_dot_1())]
}
/// Construct the recorded descriptor for this model.
fn glm_5() -> Model {
    Model {
        id: "glm-5".into(),
        name: "GLM-5".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
        max_tokens: 32_768.0,
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
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.4,
            output: 4.4,
            cache_read: 0.26,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_4() -> [(&'static str, Model); 2] {
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
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn kimi_k2_dot_6() -> Model {
    Model {
        id: "kimi-k2.6".into(),
        name: "Kimi K2.6 (3x limits)".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.32,
            output: 1.34,
            cache_read: 0.054,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_6() -> [(&'static str, Model); 4] {
    [
        ("mimo-v2-omni", mimo_v2_omni()),
        ("mimo-v2-pro", mimo_v2_pro()),
        ("mimo-v2.5", mimo_v2_dot_5()),
        ("mimo-v2.5-pro", mimo_v2_dot_5_pro()),
    ]
}
/// Construct the recorded descriptor for this model.
fn mimo_v2_omni() -> Model {
    Model {
        id: "mimo-v2-omni".into(),
        name: "MiMo V2 Omni".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mimo_v2_pro() -> Model {
    Model {
        id: "mimo-v2-pro".into(),
        name: "MiMo V2 Pro".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mimo_v2_dot_5() -> Model {
    Model {
        id: "mimo-v2.5".into(),
        name: "MiMo V2.5".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.08,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mimo_v2_dot_5_pro() -> Model {
    Model {
        id: "mimo-v2.5-pro".into(),
        name: "MiMo V2.5 Pro".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 1_048_576.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_10() -> [(&'static str, Model); 2] {
    [
        ("minimax-m2.5", minimax_m2_dot_5()),
        ("minimax-m2.7", minimax_m2_dot_7()),
    ]
}
/// Construct the recorded descriptor for this model.
fn minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
        max_tokens: 65_536.0,
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
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
pub(super) fn models_12() -> [(&'static str, Model); 2] {
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
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                thinking_format: Some(ThinkingFormat::Qwen),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn qwen3_dot_6_plus() -> Model {
    Model {
        id: "qwen3.6-plus".into(),
        name: "Qwen3.6 Plus".into(),
        api: "openai-completions".into(),
        provider: "opencode-go".into(),
        base_url: "https://opencode.ai/zen/go/v1".into(),
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
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                thinking_format: Some(ThinkingFormat::Qwen),
                ..Default::default()
            },
        ))),
    }
}
