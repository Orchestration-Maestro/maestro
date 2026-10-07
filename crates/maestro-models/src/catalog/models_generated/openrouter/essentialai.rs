// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_45() -> [(&'static str, Model); 1] {
    [("essentialai/rnj-1-instruct", essentialai_rnj_1_instruct())]
}
fn essentialai_rnj_1_instruct() -> Model {
    Model {
        id: "essentialai/rnj-1-instruct".into(),
        name: "EssentialAI: Rnj 1 Instruct".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_768.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}
