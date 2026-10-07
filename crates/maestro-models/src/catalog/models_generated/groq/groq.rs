// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_2() -> [(&'static str, Model); 2] {
    [
        ("groq/compound", groq_compound()),
        ("groq/compound-mini", groq_compound_mini()),
    ]
}
fn groq_compound() -> Model {
    Model {
        id: "groq/compound".into(),
        name: "Compound".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

fn groq_compound_mini() -> Model {
    Model {
        id: "groq/compound-mini".into(),
        name: "Compound Mini".into(),
        api: "openai-completions".into(),
        provider: "groq".into(),
        base_url: "https://api.groq.com/openai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}
