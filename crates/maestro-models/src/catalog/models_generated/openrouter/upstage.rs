// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_237() -> [(&'static str, Model); 1] {
    [("upstage/solar-pro-3", upstage_solar_pro_3())]
}
fn upstage_solar_pro_3() -> Model {
    Model {
        id: "upstage/solar-pro-3".into(),
        name: "Upstage: Solar Pro 3".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
