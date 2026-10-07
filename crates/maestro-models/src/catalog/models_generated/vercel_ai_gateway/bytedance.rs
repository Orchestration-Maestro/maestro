// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_32() -> [(&'static str, Model); 1] {
    [("bytedance/seed-1.6", bytedance_seed_1_dot_6())]
}
fn bytedance_seed_1_dot_6() -> Model {
    Model {
        id: "bytedance/seed-1.6".into(),
        name: "Seed 1.6".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.25,
            output: 2.0,
            cache_read: 0.049_999_999_999_999_996,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 32_000.0,
        headers: None,
        compat: None,
    }
}
