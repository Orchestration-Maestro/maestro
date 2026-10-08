//! Generated model descriptors for deepseek.

use crate::{
    Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel, OpenAICompletionsCompat,
    ThinkingFormat,
};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
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
        provider: "deepseek".into(),
        base_url: "https://api.deepseek.com".into(),
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
        provider: "deepseek".into(),
        base_url: "https://api.deepseek.com".into(),
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
            input: 0.435,
            output: 0.87,
            cache_read: 0.003_625,
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
