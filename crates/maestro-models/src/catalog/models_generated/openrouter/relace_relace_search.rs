// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [("relace/relace-search", relace_relace_search())]
}
fn relace_relace_search() -> Model {
    Model {
        id: "relace/relace-search".into(),
        name: "Relace: Relace Search".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
