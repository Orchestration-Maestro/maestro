// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_179() -> [(&'static str, Model); 1] {
    [("prime-intellect/intellect-3", prime_intellect_intellect_3())]
}
fn prime_intellect_intellect_3() -> Model {
    Model {
        id: "prime-intellect/intellect-3".into(),
        name: "Prime Intellect: INTELLECT-3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.199_999_999_999_999_98,
            output: 1.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
