// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models_107() -> [(&'static str, Model); 1] {
    [(
        "nex-agi/deepseek-v3.1-nex-n1",
        nex_agi_deepseek_v3_dot_1_nex_n1(),
    )]
}
fn nex_agi_deepseek_v3_dot_1_nex_n1() -> Model {
    Model {
        id: "nex-agi/deepseek-v3.1-nex-n1".into(),
        name: "Nex AGI: DeepSeek V3.1 Nex N1".into(),
        api: "openai-completions".into(),
        provider: "openrouter".into(),
        base_url: "https://openrouter.ai/api/v1".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.135,
            output: 0.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 163_840.0,
        headers: None,
        compat: None,
    }
}
