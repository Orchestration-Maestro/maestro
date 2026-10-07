// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_11() -> [(&'static str, Model); 10] {
    [
        ("mistral-large-2411", mistral_large_2411()),
        ("mistral-large-2512", mistral_large_2512()),
        ("mistral-large-latest", mistral_large_latest()),
        ("mistral-medium-2505", mistral_medium_2505()),
        ("mistral-medium-2508", mistral_medium_2508()),
        ("mistral-medium-2604", mistral_medium_2604()),
        ("mistral-medium-3.5", mistral_medium_3_dot_5()),
        ("mistral-medium-latest", mistral_medium_latest()),
        ("mistral-nemo", mistral_nemo()),
        ("mistral-small-2506", mistral_small_2506()),
    ]
}
pub(super) fn models_21() -> [(&'static str, Model); 3] {
    [
        ("mistral-small-2603", mistral_small_2603()),
        ("mistral-small-latest", mistral_small_latest()),
        ("open-mistral-7b", open_mistral_7b()),
    ]
}
fn mistral_large_2411() -> Model {
    Model {
        id: "mistral-large-2411".into(),
        name: "Mistral Large 2.1".into(),
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
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn mistral_large_2512() -> Model {
    Model {
        id: "mistral-large-2512".into(),
        name: "Mistral Large 3".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn mistral_large_latest() -> Model {
    Model {
        id: "mistral-large-latest".into(),
        name: "Mistral Large (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn mistral_medium_2505() -> Model {
    Model {
        id: "mistral-medium-2505".into(),
        name: "Mistral Medium 3".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn mistral_medium_2508() -> Model {
    Model {
        id: "mistral-medium-2508".into(),
        name: "Mistral Medium 3.1".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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

fn mistral_medium_2604() -> Model {
    Model {
        id: "mistral-medium-2604".into(),
        name: "Mistral Medium 3.5".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.5,
            output: 7.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn mistral_medium_3_dot_5() -> Model {
    Model {
        id: "mistral-medium-3.5".into(),
        name: "Mistral Medium 3.5".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.5,
            output: 7.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn mistral_medium_latest() -> Model {
    Model {
        id: "mistral-medium-latest".into(),
        name: "Mistral Medium (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 1.5,
            output: 7.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}

fn mistral_nemo() -> Model {
    Model {
        id: "mistral-nemo".into(),
        name: "Mistral Nemo".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_small_2506() -> Model {
    Model {
        id: "mistral-small-2506".into(),
        name: "Mistral Small 3.2".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn mistral_small_2603() -> Model {
    Model {
        id: "mistral-small-2603".into(),
        name: "Mistral Small 4".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

fn mistral_small_latest() -> Model {
    Model {
        id: "mistral-small-latest".into(),
        name: "Mistral Small (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
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
        context_window: 8000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}
