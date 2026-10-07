// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 2] {
    [
        ("kimi-for-coding", kimi_for_coding()),
        ("kimi-k2-thinking", kimi_k2_thinking()),
    ]
}
fn kimi_for_coding() -> Model {
    Model {
        id: "kimi-for-coding".into(),
        name: "Kimi For Coding".into(),
        api: "anthropic-messages".into(),
        provider: "kimi-coding".into(),
        base_url: "https://api.kimi.com/coding".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: Some([("User-Agent".into(), "KimiCLI/1.5".into())].into()),
        compat: None,
    }
}

fn kimi_k2_thinking() -> Model {
    Model {
        id: "kimi-k2-thinking".into(),
        name: "Kimi K2 Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "kimi-coding".into(),
        base_url: "https://api.kimi.com/coding".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: Some([("User-Agent".into(), "KimiCLI/1.5".into())].into()),
        compat: None,
    }
}
