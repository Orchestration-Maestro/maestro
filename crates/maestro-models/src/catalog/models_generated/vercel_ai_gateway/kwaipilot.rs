// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_55() -> [(&'static str, Model); 1] {
    [("kwaipilot/kat-coder-pro-v2", kwaipilot_kat_coder_pro_v2())]
}
fn kwaipilot_kat_coder_pro_v2() -> Model {
    Model {
        id: "kwaipilot/kat-coder-pro-v2".into(),
        name: "Kat Coder Pro V2".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}
