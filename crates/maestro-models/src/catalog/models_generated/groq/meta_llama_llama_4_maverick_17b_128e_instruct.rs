// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        (
            "meta-llama/llama-4-maverick-17b-128e-instruct",
            meta_llama_llama_4_maverick_17b_128e_instruct(),
        ),
        (
            "meta-llama/llama-4-scout-17b-16e-instruct",
            meta_llama_llama_4_scout_17b_16e_instruct(),
        ),
    ]
}
fn meta_llama_llama_4_maverick_17b_128e_instruct() -> Model {
    Model {
        id: "meta-llama/llama-4-maverick-17b-128e-instruct".into(),
        name: "Llama 4 Maverick 17B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}

fn meta_llama_llama_4_scout_17b_16e_instruct() -> Model {
    Model {
        id: "meta-llama/llama-4-scout-17b-16e-instruct".into(),
        name: "Llama 4 Scout 17B".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.11,
            output: 0.34,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8_192.0,
        headers: None,
        compat: None,
    }
}
