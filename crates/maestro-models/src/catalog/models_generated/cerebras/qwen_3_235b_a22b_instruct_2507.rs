// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [(
        "qwen-3-235b-a22b-instruct-2507",
        qwen_3_235b_a22b_instruct_2507(),
    )]
}
fn qwen_3_235b_a22b_instruct_2507() -> Model {
    Model {
        id: "qwen-3-235b-a22b-instruct-2507".into(),
        name: "Qwen 3 235B Instruct".into(),
        api: "openai-completions".into(),
        provider: "cerebras".into(),
        base_url: "https://api.cerebras.ai/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}
