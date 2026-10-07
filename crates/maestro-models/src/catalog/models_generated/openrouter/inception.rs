// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_65() -> [(&'static str, Model); 1] {
    [("inception/mercury-2", inception_mercury_2())]
}
fn inception_mercury_2() -> Model {
    Model {
        id: "inception/mercury-2".into(),
        name: "Inception: Mercury 2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.75,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 50_000.0,
        headers: None,
        compat: None,
    }
}
