// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_7() -> [(&'static str, Model); 2] {
    [
        ("magistral-medium-latest", magistral_medium_latest()),
        ("magistral-small", magistral_small()),
    ]
}
fn magistral_medium_latest() -> Model {
    Model {
        id: "magistral-medium-latest".into(),
        name: "Magistral Medium (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 5.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn magistral_small() -> Model {
    Model {
        id: "magistral-small".into(),
        name: "Magistral Small".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
