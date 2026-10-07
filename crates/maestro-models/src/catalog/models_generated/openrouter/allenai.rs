// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_2() -> [(&'static str, Model); 1] {
    [(
        "allenai/olmo-3.1-32b-instruct",
        allenai_olmo_3_dot_1_32b_instruct(),
    )]
}
fn allenai_olmo_3_dot_1_32b_instruct() -> Model {
    Model {
        id: "allenai/olmo-3.1-32b-instruct".into(),
        name: "AllenAI: Olmo 3.1 32B Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 65_536.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
