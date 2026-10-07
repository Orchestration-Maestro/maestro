// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [("kwaipilot/kat-coder-pro-v2", kwaipilot_kat_coder_pro_v2())]
}
fn kwaipilot_kat_coder_pro_v2() -> Model {
    Model {
        id: "kwaipilot/kat-coder-pro-v2".into(),
        name: "Kwaipilot: KAT-Coder-Pro V2".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.06,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 80_000.0,
        headers: None,
        compat: None,
    }
}
