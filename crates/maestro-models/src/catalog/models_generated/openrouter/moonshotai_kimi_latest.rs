// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [("~moonshotai/kimi-latest", moonshotai_kimi_latest())]
}
fn moonshotai_kimi_latest() -> Model {
    Model {
        id: "~moonshotai/kimi-latest".into(),
        name: "MoonshotAI Kimi Latest".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.74,
            output: 3.49,
            cache_read: 0.14,
            cache_write: 0.0,
        },
        context_window: 262_142.0,
        max_tokens: 262_142.0,
        headers: None,
        compat: None,
    }
}
