// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        ("sao10k/l3-euryale-70b", sao10k_l3_euryale_70b()),
        ("sao10k/l3.1-euryale-70b", sao10k_l3_dot_1_euryale_70b()),
    ]
}
fn sao10k_l3_euryale_70b() -> Model {
    Model {
        id: "sao10k/l3-euryale-70b".into(),
        name: "Sao10k: Llama 3 Euryale 70B v2.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.48,
            output: 1.48,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_192.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn sao10k_l3_dot_1_euryale_70b() -> Model {
    Model {
        id: "sao10k/l3.1-euryale-70b".into(),
        name: "Sao10K: Llama 3.1 Euryale 70B v2.2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.85,
            output: 0.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
