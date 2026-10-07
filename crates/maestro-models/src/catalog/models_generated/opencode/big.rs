// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [("big-pickle", big_pickle())]
}
fn big_pickle() -> Model {
    Model {
        id: "big-pickle".into(),
        name: "Big Pickle".into(),
        api: "anthropic-messages".into(),
        provider: "opencode".into(),
        base_url: "https://opencode.ai/zen".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}
