// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_4() -> [(&'static str, Model); 4] {
    [
        ("llama-3.1-8b-instant", llama_3_dot_1_8b_instant()),
        ("llama-3.3-70b-versatile", llama_3_dot_3_70b_versatile()),
        ("llama3-70b-8192", llama3_70b_8192()),
        ("llama3-8b-8192", llama3_8b_8192()),
    ]
}
fn llama_3_dot_1_8b_instant() -> Model {
    Model {
        id: "llama-3.1-8b-instant".into(),
        name: "Llama 3.1 8B Instant".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.05,
            output: 0.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

fn llama_3_dot_3_70b_versatile() -> Model {
    Model {
        id: "llama-3.3-70b-versatile".into(),
        name: "Llama 3.3 70B Versatile".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.59,
            output: 0.79,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn llama3_70b_8192() -> Model {
    Model {
        id: "llama3-70b-8192".into(),
        name: "Llama 3 70B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.59,
            output: 0.79,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn llama3_8b_8192() -> Model {
    Model {
        id: "llama3-8b-8192".into(),
        name: "Llama 3 8B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.05,
            output: 0.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 8192.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}
