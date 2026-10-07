// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
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
