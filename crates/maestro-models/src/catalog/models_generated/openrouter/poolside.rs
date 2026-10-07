// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_177() -> [(&'static str, Model); 2] {
    [
        ("poolside/laguna-m.1:free", poolside_laguna_m_dot_1_free()),
        ("poolside/laguna-xs.2:free", poolside_laguna_xs_dot_2_free()),
    ]
}
fn poolside_laguna_m_dot_1_free() -> Model {
    Model {
        id: "poolside/laguna-m.1:free".into(),
        name: "Poolside: Laguna M.1 (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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

fn poolside_laguna_xs_dot_2_free() -> Model {
    Model {
        id: "poolside/laguna-xs.2:free".into(),
        name: "Poolside: Laguna XS.2 (free)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
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
