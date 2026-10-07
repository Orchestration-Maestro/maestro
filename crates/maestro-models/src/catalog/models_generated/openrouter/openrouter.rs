// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_174() -> [(&'static str, Model); 3] {
    [
        ("openrouter/auto", openrouter_auto()),
        ("openrouter/free", openrouter_free()),
        ("openrouter/owl-alpha", openrouter_owl_alpha()),
    ]
}
fn openrouter_auto() -> Model {
    Model {
        id: "openrouter/auto".into(),
        name: "Auto Router".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: -1_000_000.0,
            output: -1_000_000.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 2_000_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn openrouter_free() -> Model {
    Model {
        id: "openrouter/free".into(),
        name: "Free Models Router".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

fn openrouter_owl_alpha() -> Model {
    Model {
        id: "openrouter/owl-alpha".into(),
        name: "Owl Alpha".into(),
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
        context_window: 1_048_756.0,
        max_tokens: 262_144.0,
        headers: None,
        compat: None,
    }
}
