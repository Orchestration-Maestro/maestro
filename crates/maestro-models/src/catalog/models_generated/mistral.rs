//! Generated model descriptors for mistral.

use crate::{Model, ModelCost, ModelInput};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_1());
    models.extend(models_7());
    models.extend(models_9());
    models.extend(models_11());
    models.extend(models_21());
    models.extend(models_24());
    models.extend(models_26());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [("codestral-latest", codestral_latest())]
}
/// Construct the recorded descriptor for this model.
fn codestral_latest() -> Model {
    Model {
        id: "codestral-latest".into(),
        name: "Codestral (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 0.9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
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
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_7() -> [(&'static str, Model); 2] {
    [
        ("magistral-medium-latest", magistral_medium_latest()),
        ("magistral-small", magistral_small()),
    ]
}
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_9() -> [(&'static str, Model); 2] {
    [
        ("ministral-3b-latest", ministral_3b_latest()),
        ("ministral-8b-latest", ministral_8b_latest()),
    ]
}
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
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
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_21() -> [(&'static str, Model); 3] {
    [
        ("mistral-small-2603", mistral_small_2603()),
        ("mistral-small-latest", mistral_small_latest()),
        ("open-mistral-7b", open_mistral_7b()),
    ]
}
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_24() -> [(&'static str, Model); 2] {
    [
        ("open-mixtral-8x22b", open_mixtral_8x22b()),
        ("open-mixtral-8x7b", open_mixtral_8x7b()),
    ]
}
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_26() -> [(&'static str, Model); 2] {
    [
        ("pixtral-12b", pixtral_12b()),
        ("pixtral-large-latest", pixtral_large_latest()),
    ]
}
/// Construct the recorded descriptor for this model.
fn pixtral_12b() -> Model {
    Model {
        id: "pixtral-12b".into(),
        name: "Pixtral 12B".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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

/// Construct the recorded descriptor for this model.
fn pixtral_large_latest() -> Model {
    Model {
        id: "pixtral-large-latest".into(),
        name: "Pixtral Large (latest)".into(),
        api: "mistral-conversations".into(),
        provider: "mistral".into(),
        base_url: "https://api.mistral.ai".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
