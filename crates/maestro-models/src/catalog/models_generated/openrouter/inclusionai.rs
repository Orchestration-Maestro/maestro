// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_66() -> [(&'static str, Model); 2] {
    [
        (
            "inclusionai/ling-2.6-1t:free",
            inclusionai_ling_2_dot_6_1t_free(),
        ),
        (
            "inclusionai/ling-2.6-flash",
            inclusionai_ling_2_dot_6_flash(),
        ),
    ]
}
fn inclusionai_ling_2_dot_6_1t_free() -> Model {
    Model {
        id: "inclusionai/ling-2.6-1t:free".into(),
        name: "inclusionAI: Ling-2.6-1T (free)".into(),
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
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn inclusionai_ling_2_dot_6_flash() -> Model {
    Model {
        id: "inclusionai/ling-2.6-flash".into(),
        name: "inclusionAI: Ling-2.6-flash".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.08,
            output: 0.24,
            cache_read: 0.016,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
