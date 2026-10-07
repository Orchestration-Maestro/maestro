// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [(
        "ibm-granite/granite-4.1-8b",
        ibm_granite_granite_4_dot_1_8b(),
    )]
}
fn ibm_granite_granite_4_dot_1_8b() -> Model {
    Model {
        id: "ibm-granite/granite-4.1-8b".into(),
        name: "IBM: Granite 4.1 8B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.049_999_999_999_999_996,
            output: 0.099_999_999_999_999_99,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}
