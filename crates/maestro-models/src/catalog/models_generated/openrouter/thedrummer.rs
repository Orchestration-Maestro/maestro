// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_234() -> [(&'static str, Model); 2] {
    [
        ("thedrummer/rocinante-12b", thedrummer_rocinante_12b()),
        ("thedrummer/unslopnemo-12b", thedrummer_unslopnemo_12b()),
    ]
}
fn thedrummer_rocinante_12b() -> Model {
    Model {
        id: "thedrummer/rocinante-12b".into(),
        name: "TheDrummer: Rocinante 12B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.169_999_999_999_999_98,
            output: 0.43,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

fn thedrummer_unslopnemo_12b() -> Model {
    Model {
        id: "thedrummer/unslopnemo-12b".into(),
        name: "TheDrummer: UnslopNemo 12B".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.399_999_999_999_999_97,
            output: 0.399_999_999_999_999_97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
