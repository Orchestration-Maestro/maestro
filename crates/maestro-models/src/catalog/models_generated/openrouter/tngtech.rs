// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_236() -> [(&'static str, Model); 1] {
    [(
        "tngtech/deepseek-r1t2-chimera",
        tngtech_deepseek_r1t2_chimera(),
    )]
}
fn tngtech_deepseek_r1t2_chimera() -> Model {
    Model {
        id: "tngtech/deepseek-r1t2-chimera".into(),
        name: "TNG: DeepSeek R1T2 Chimera".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.1,
            cache_read: 0.15,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 163_840.0,
        headers: None,
        compat: None,
    }
}
