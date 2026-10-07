// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_9() -> [(&'static str, Model); 2] {
    [
        ("ministral-3b-latest", ministral_3b_latest()),
        ("ministral-8b-latest", ministral_8b_latest()),
    ]
}
fn ministral_3b_latest() -> Model {
    Model {
        id: "ministral-3b-latest".into(),
        name: "Ministral 3B (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.04,
            output: 0.04,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn ministral_8b_latest() -> Model {
    Model {
        id: "ministral-8b-latest".into(),
        name: "Ministral 8B (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
