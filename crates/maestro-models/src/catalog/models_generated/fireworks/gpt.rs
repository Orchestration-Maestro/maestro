// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_8() -> [(&'static str, Model); 2] {
    [
        (
            "accounts/fireworks/models/gpt-oss-120b",
            accounts_fireworks_models_gpt_oss_120b(),
        ),
        (
            "accounts/fireworks/models/gpt-oss-20b",
            accounts_fireworks_models_gpt_oss_20b(),
        ),
    ]
}
fn accounts_fireworks_models_gpt_oss_120b() -> Model {
    Model {
        id: "accounts/fireworks/models/gpt-oss-120b".into(),
        name: "GPT OSS 120B".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
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
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn accounts_fireworks_models_gpt_oss_20b() -> Model {
    Model {
        id: "accounts/fireworks/models/gpt-oss-20b".into(),
        name: "GPT OSS 20B".into(),
        api: "anthropic-messages".into(),
        provider: "fireworks".into(),
        base_url: "https://api.fireworks.ai/inference".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.05,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
