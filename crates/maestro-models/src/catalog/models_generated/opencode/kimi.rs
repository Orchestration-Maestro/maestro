// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_30() -> [(&'static str, Model); 2] {
    [
        ("kimi-k2.5", kimi_k2_dot_5()),
        ("kimi-k2.6", kimi_k2_dot_6()),
    ]
}
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
