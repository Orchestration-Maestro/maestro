//! Generated model descriptors for openai-codex.

use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 10] {
    [
        ("gpt-5.1", gpt_5_dot_1()),
        ("gpt-5.1-codex-max", gpt_5_dot_1_codex_max()),
        ("gpt-5.1-codex-mini", gpt_5_dot_1_codex_mini()),
        ("gpt-5.2", gpt_5_dot_2()),
        ("gpt-5.2-codex", gpt_5_dot_2_codex()),
        ("gpt-5.3-codex", gpt_5_dot_3_codex()),
        ("gpt-5.3-codex-spark", gpt_5_dot_3_codex_spark()),
        ("gpt-5.4", gpt_5_dot_4()),
        ("gpt-5.4-mini", gpt_5_dot_4_mini()),
        ("gpt-5.5", gpt_5_dot_5()),
    ]
}
/// Construct the recorded descriptor for this model.
fn gpt_5_dot_1() -> Model {
    Model {
        id: "gpt-5.1".into(),
        name: "GPT-5.1".into(),
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.25,
            output: 10.0,
            cache_read: 0.125,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Minimal, Some("medium".into())),
                (ModelThinkingLevel::Low, Some("medium".into())),
                (ModelThinkingLevel::Medium, Some("medium".into())),
                (ModelThinkingLevel::High, Some("high".into())),
            ]
            .into(),
        ),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.025,
            cache_write: 0.0,
        },
        context_window: 272_000.0,
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
                (ModelThinkingLevel::Minimal, Some("low".into())),
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
        context_window: 272_000.0,
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
                (ModelThinkingLevel::Minimal, Some("low".into())),
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
        context_window: 272_000.0,
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
                (ModelThinkingLevel::Minimal, Some("low".into())),
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
        context_window: 272_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn gpt_5_dot_3_codex_spark() -> Model {
    Model {
        id: "gpt-5.3-codex-spark".into(),
        name: "GPT-5.3 Codex Spark".into(),
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
                (ModelThinkingLevel::Minimal, Some("low".into())),
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
        context_window: 128_000.0,
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
                (ModelThinkingLevel::Minimal, Some("low".into())),
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
                (ModelThinkingLevel::Minimal, Some("low".into())),
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
        context_window: 272_000.0,
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
        api: "openai-codex-responses".into(),
        provider: "openai-codex".into(),
        base_url: "https://chatgpt.com/backend-api".into(),
        reasoning: true,
        thinking_level_map: Some(
            [
                (ModelThinkingLevel::Xhigh, Some("xhigh".into())),
                (ModelThinkingLevel::Minimal, Some("low".into())),
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
        context_window: 272_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
