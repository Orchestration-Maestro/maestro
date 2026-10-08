//! Generated model descriptors for zai.

use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat, ThinkingFormat};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 5] {
    [
        ("glm-4.5-air", glm_4_dot_5_air()),
        ("glm-4.7", glm_4_dot_7()),
        ("glm-5-turbo", glm_5_turbo()),
        ("glm-5.1", glm_5_dot_1()),
        ("glm-5v-turbo", glm_5v_turbo()),
    ]
}
/// Construct the recorded descriptor for this model.
fn glm_4_dot_5_air() -> Model {
    Model {
        id: "glm-4.5-air".into(),
        name: "GLM-4.5-Air".into(),
        api: "openai-completions".into(),
        provider: "zai".into(),
        base_url: "https://api.z.ai/api/coding/paas/v4".into(),
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
        max_tokens: 98_304.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                thinking_format: Some(ThinkingFormat::Zai),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn glm_4_dot_7() -> Model {
    Model {
        id: "glm-4.7".into(),
        name: "GLM-4.7".into(),
        api: "openai-completions".into(),
        provider: "zai".into(),
        base_url: "https://api.z.ai/api/coding/paas/v4".into(),
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
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                thinking_format: Some(ThinkingFormat::Zai),
                zai_tool_stream: Some(true),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn glm_5_turbo() -> Model {
    Model {
        id: "glm-5-turbo".into(),
        name: "GLM-5-Turbo".into(),
        api: "openai-completions".into(),
        provider: "zai".into(),
        base_url: "https://api.z.ai/api/coding/paas/v4".into(),
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
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                thinking_format: Some(ThinkingFormat::Zai),
                zai_tool_stream: Some(true),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn glm_5_dot_1() -> Model {
    Model {
        id: "glm-5.1".into(),
        name: "GLM-5.1".into(),
        api: "openai-completions".into(),
        provider: "zai".into(),
        base_url: "https://api.z.ai/api/coding/paas/v4".into(),
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
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                thinking_format: Some(ThinkingFormat::Zai),
                zai_tool_stream: Some(true),
                ..Default::default()
            },
        ))),
    }
}

/// Construct the recorded descriptor for this model.
fn glm_5v_turbo() -> Model {
    Model {
        id: "glm-5v-turbo".into(),
        name: "GLM-5V-Turbo".into(),
        api: "openai-completions".into(),
        provider: "zai".into(),
        base_url: "https://api.z.ai/api/coding/paas/v4".into(),
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
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                thinking_format: Some(ThinkingFormat::Zai),
                zai_tool_stream: Some(true),
                ..Default::default()
            },
        ))),
    }
}
