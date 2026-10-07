// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 4] {
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
