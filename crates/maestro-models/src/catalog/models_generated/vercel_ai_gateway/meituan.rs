// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_56() -> [(&'static str, Model); 1] {
    [("meituan/longcat-flash-chat", meituan_longcat_flash_chat())]
}
fn meituan_longcat_flash_chat() -> Model {
    Model {
        id: "meituan/longcat-flash-chat".into(),
        name: "LongCat Flash Chat".into(),
        api: "anthropic-messages".into(),
        provider: "vercel-ai-gateway".into(),
        base_url: "https://ai-gateway.vercel.sh".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 100_000.0,
        headers: None,
        compat: None,
    }
}
