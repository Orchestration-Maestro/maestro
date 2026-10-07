// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [(
        "mistralai/voxtral-small-24b-2507",
        mistralai_voxtral_small_24b_2507(),
    )]
}
fn mistralai_voxtral_small_24b_2507() -> Model {
    Model {
        id: "mistralai/voxtral-small-24b-2507".into(),
        name: "Mistral: Voxtral Small 24B 2507".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.099_999_999_999_999_99,
            output: 0.3,
            cache_read: 0.01,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 4_096.0,
        headers: None,
        compat: None,
    }
}
