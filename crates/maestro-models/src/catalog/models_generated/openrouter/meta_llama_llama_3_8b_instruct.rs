// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 6] {
    [
        (
            "meta-llama/llama-3-8b-instruct",
            meta_llama_llama_3_8b_instruct(),
        ),
        (
            "meta-llama/llama-3.1-70b-instruct",
            meta_llama_llama_3_dot_1_70b_instruct(),
        ),
        (
            "meta-llama/llama-3.1-8b-instruct",
            meta_llama_llama_3_dot_1_8b_instruct(),
        ),
        (
            "meta-llama/llama-3.3-70b-instruct",
            meta_llama_llama_3_dot_3_70b_instruct(),
        ),
        (
            "meta-llama/llama-3.3-70b-instruct:free",
            meta_llama_llama_3_dot_3_70b_instruct_free(),
        ),
        ("meta-llama/llama-4-scout", meta_llama_llama_4_scout()),
    ]
}
fn meta_llama_llama_3_8b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3-8b-instruct".into(),
        name: "Meta: Llama 3 8B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.03,
            output: 0.04,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8_192.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_llama_3_dot_1_70b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3.1-70b-instruct".into(),
        name: "Meta: Llama 3.1 70B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_llama_3_dot_1_8b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3.1-8b-instruct".into(),
        name: "Meta: Llama 3.1 8B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.02,
            output: 0.049_999_999_999_999_996,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_384.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_llama_3_dot_3_70b_instruct() -> Model {
    Model {
        id: "meta-llama/llama-3.3-70b-instruct".into(),
        name: "Meta: Llama 3.3 70B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.32,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_llama_3_dot_3_70b_instruct_free() -> Model {
    Model {
        id: "meta-llama/llama-3.3-70b-instruct:free".into(),
        name: "Meta: Llama 3.3 70B Instruct (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 65_536.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_llama_4_scout() -> Model {
    Model {
        id: "meta-llama/llama-4-scout".into(),
        name: "Meta: Llama 4 Scout".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.08,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 327_680.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
