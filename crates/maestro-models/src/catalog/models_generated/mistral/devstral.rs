// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_1() -> [(&'static str, Model); 6] {
    [
        ("devstral-2512", devstral_2512()),
        ("devstral-medium-2507", devstral_medium_2507()),
        ("devstral-medium-latest", devstral_medium_latest()),
        ("devstral-small-2505", devstral_small_2505()),
        ("devstral-small-2507", devstral_small_2507()),
        ("labs-devstral-small-2512", labs_devstral_small_2512()),
    ]
}
fn devstral_2512() -> Model {
    Model {
        id: "devstral-2512".into(),
        name: "Devstral 2".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn devstral_medium_2507() -> Model {
    Model {
        id: "devstral-medium-2507".into(),
        name: "Devstral Medium".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn devstral_medium_latest() -> Model {
    Model {
        id: "devstral-medium-latest".into(),
        name: "Devstral 2 (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn devstral_small_2505() -> Model {
    Model {
        id: "devstral-small-2505".into(),
        name: "Devstral Small 2505".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn devstral_small_2507() -> Model {
    Model {
        id: "devstral-small-2507".into(),
        name: "Devstral Small".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn labs_devstral_small_2512() -> Model {
    Model {
        id: "labs-devstral-small-2512".into(),
        name: "Devstral Small 2".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}
