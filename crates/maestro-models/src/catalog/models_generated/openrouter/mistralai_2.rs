// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_100() -> [(&'static str, Model); 2] {
    [
        (
            "mistralai/pixtral-large-2411",
            mistralai_pixtral_large_2411(),
        ),
        (
            "mistralai/voxtral-small-24b-2507",
            mistralai_voxtral_small_24b_2507(),
        ),
    ]
}
fn mistralai_pixtral_large_2411() -> Model {
    Model {
        id: "mistralai/pixtral-large-2411".into(),
        name: "Mistral: Pixtral Large 2411".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.199_999_999_999_999_98,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
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
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
