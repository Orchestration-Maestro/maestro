// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_33() -> [(&'static str, Model); 1] {
    [("cohere/command-a", cohere_command_a())]
}
fn cohere_command_a() -> Model {
    Model {
        id: "cohere/command-a".into(),
        name: "Command A".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8000.0,
        headers: None,
        compat: None,
    }
}
