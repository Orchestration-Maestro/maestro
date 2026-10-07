// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_62() -> [(&'static str, Model); 1] {
    [("moonshot.kimi-k2-thinking", moonshot_dot_kimi_k2_thinking())]
}
fn moonshot_dot_kimi_k2_thinking() -> Model {
    Model {
        id: "moonshot.kimi-k2-thinking".into(),
        name: "Kimi K2 Thinking".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}
