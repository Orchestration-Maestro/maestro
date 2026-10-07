// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 3] {
    [
        ("open-mistral-7b", open_mistral_7b()),
        ("open-mixtral-8x22b", open_mixtral_8x22b()),
        ("open-mixtral-8x7b", open_mixtral_8x7b()),
    ]
}
fn open_mistral_7b() -> Model {
    Model {
        id: "open-mistral-7b".into(),
        name: "Mistral 7B".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.25,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_000.0,
        max_tokens: 8_000.0,
        headers: None,
        compat: None,
    }
}

fn open_mixtral_8x22b() -> Model {
    Model {
        id: "open-mixtral-8x22b".into(),
        name: "Mixtral 8x22B".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 64_000.0,
        max_tokens: 64_000.0,
        headers: None,
        compat: None,
    }
}

fn open_mixtral_8x7b() -> Model {
    Model {
        id: "open-mixtral-8x7b".into(),
        name: "Mixtral 8x7B".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.7,
            output: 0.7,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}
