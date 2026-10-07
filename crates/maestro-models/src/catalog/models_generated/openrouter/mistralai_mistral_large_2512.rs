// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        (
            "mistralai/mistral-large-2512",
            mistralai_mistral_large_2512(),
        ),
        ("mistralai/mistral-medium-3", mistralai_mistral_medium_3()),
        (
            "mistralai/mistral-medium-3.1",
            mistralai_mistral_medium_3_dot_1(),
        ),
        ("mistralai/mistral-nemo", mistralai_mistral_nemo()),
        ("mistralai/mistral-saba", mistralai_mistral_saba()),
        (
            "mistralai/mistral-small-2603",
            mistralai_mistral_small_2603(),
        ),
        (
            "mistralai/mistral-small-3.2-24b-instruct",
            mistralai_mistral_small_3_dot_2_24b_instruct(),
        ),
        (
            "mistralai/mixtral-8x22b-instruct",
            mistralai_mixtral_8x22b_instruct(),
        ),
        (
            "mistralai/mixtral-8x7b-instruct",
            mistralai_mixtral_8x7b_instruct(),
        ),
        (
            "mistralai/pixtral-large-2411",
            mistralai_pixtral_large_2411(),
        ),
    ]
}
fn mistralai_mistral_large_2512() -> Model {
    Model {
        id: "mistralai/mistral-large-2512".into(),
        name: "Mistral: Mistral Large 3 2512".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mistral_medium_3() -> Model {
    Model {
        id: "mistralai/mistral-medium-3".into(),
        name: "Mistral: Mistral Medium 3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mistral_medium_3_dot_1() -> Model {
    Model {
        id: "mistralai/mistral-medium-3.1".into(),
        name: "Mistral: Mistral Medium 3.1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 2.0,
            cache_read: 0.04,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mistral_nemo() -> Model {
    Model {
        id: "mistralai/mistral-nemo".into(),
        name: "Mistral: Mistral Nemo".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.02,
            output: 0.03,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mistral_saba() -> Model {
    Model {
        id: "mistralai/mistral-saba".into(),
        name: "Mistral: Saba".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.6,
            cache_read: 0.02,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mistral_small_2603() -> Model {
    Model {
        id: "mistralai/mistral-small-2603".into(),
        name: "Mistral: Mistral Small 4".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mistral_small_3_dot_2_24b_instruct() -> Model {
    Model {
        id: "mistralai/mistral-small-3.2-24b-instruct".into(),
        name: "Mistral: Mistral Small 3.2 24B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.075,
            output: 0.199_999_999_999_999_98,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mixtral_8x22b_instruct() -> Model {
    Model {
        id: "mistralai/mixtral-8x22b-instruct".into(),
        name: "Mistral: Mixtral 8x22B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 65_536.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_mixtral_8x7b_instruct() -> Model {
    Model {
        id: "mistralai/mixtral-8x7b-instruct".into(),
        name: "Mistral: Mixtral 8x7B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.54,
            output: 0.54,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn mistralai_pixtral_large_2411() -> Model {
    Model {
        id: "mistralai/pixtral-large-2411".into(),
        name: "Mistral: Pixtral Large 2411".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}
