// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_33() -> [(&'static str, Model); 2] {
    [
        ("cohere/command-r-08-2024", cohere_command_r_08_2024()),
        (
            "cohere/command-r-plus-08-2024",
            cohere_command_r_plus_08_2024(),
        ),
    ]
}
fn cohere_command_r_08_2024() -> Model {
    Model {
        id: "cohere/command-r-08-2024".into(),
        name: "Cohere: Command R (08-2024)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}

fn cohere_command_r_plus_08_2024() -> Model {
    Model {
        id: "cohere/command-r-plus-08-2024".into(),
        name: "Cohere: Command R+ (08-2024)".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4000.0,
        headers: None,
        compat: None,
    }
}
