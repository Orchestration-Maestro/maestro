// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
        ("inception/mercury-2", inception_mercury_2()),
        (
            "inception/mercury-coder-small",
            inception_mercury_coder_small(),
        ),
    ]
}
fn inception_mercury_2() -> Model {
    Model {
        id: "inception/mercury-2".into(),
        name: "Mercury 2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 0.75,
            cache_read: 0.024_999_999_999_999_998,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

fn inception_mercury_coder_small() -> Model {
    Model {
        id: "inception/mercury-coder-small".into(),
        name: "Mercury Coder Small Beta".into(),
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
        context_window: 32_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}
