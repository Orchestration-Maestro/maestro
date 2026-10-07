// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [("gpt-oss-120b", gpt_oss_120b())]
}
fn gpt_oss_120b() -> Model {
    Model {
        id: "gpt-oss-120b".into(),
        name: "GPT OSS 120B".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.69,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}
