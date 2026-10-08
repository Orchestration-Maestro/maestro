//! Generated model descriptors for cerebras.

use crate::{Model, ModelCost, ModelInput};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_1());
    models.extend(models_2());
    models.extend(models_3());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [("gpt-oss-120b", gpt_oss_120b())]
}
/// Construct the recorded descriptor for this model.
fn gpt_oss_120b() -> Model {
    Model {
        id: "gpt-oss-120b".into(),
        name: "GPT OSS 120B".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.69,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_1() -> [(&'static str, Model); 1] {
    [("llama3.1-8b", llama3_dot_1_8b())]
}
/// Construct the recorded descriptor for this model.
fn llama3_dot_1_8b() -> Model {
    Model {
        id: "llama3.1-8b".into(),
        name: "Llama 3.1 8B".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_2() -> [(&'static str, Model); 1] {
    [(
        "qwen-3-235b-a22b-instruct-2507",
        qwen_3_235b_a22b_instruct_2507(),
    )]
}
/// Construct the recorded descriptor for this model.
fn qwen_3_235b_a22b_instruct_2507() -> Model {
    Model {
        id: "qwen-3-235b-a22b-instruct-2507".into(),
        name: "Qwen 3 235B Instruct".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_3() -> [(&'static str, Model); 1] {
    [("zai-glm-4.7", zai_glm_4_dot_7())]
}
/// Construct the recorded descriptor for this model.
fn zai_glm_4_dot_7() -> Model {
    Model {
        id: "zai-glm-4.7".into(),
        name: "Z.AI GLM-4.7".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.25,
            output: 2.75,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}
