// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_30() -> [(&'static str, Model); 2] {
    [
        (
            "arcee-ai/trinity-large-preview",
            arcee_ai_trinity_large_preview(),
        ),
        (
            "arcee-ai/trinity-large-thinking",
            arcee_ai_trinity_large_thinking(),
        ),
    ]
}
fn arcee_ai_trinity_large_preview() -> Model {
    Model {
        id: "arcee-ai/trinity-large-preview".into(),
        name: "Trinity Large Preview".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 1.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 131_000.0,
        headers: None,
        compat: None,
    }
}

fn arcee_ai_trinity_large_thinking() -> Model {
    Model {
        id: "arcee-ai/trinity-large-thinking".into(),
        name: "Trinity Large Thinking".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.899_999_999_999_999_9,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_100.0,
        max_tokens: 80_000.0,
        headers: None,
        compat: None,
    }
}
