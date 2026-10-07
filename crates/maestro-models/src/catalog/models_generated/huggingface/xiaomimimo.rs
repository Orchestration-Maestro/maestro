// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_9() -> [(&'static str, Model); 1] {
    [("XiaomiMiMo/MiMo-V2-Flash", xiaomimimo_mimo_v2_flash())]
}
fn xiaomimimo_mimo_v2_flash() -> Model {
    Model {
        id: "XiaomiMiMo/MiMo-V2-Flash".into(),
        name: "MiMo-V2-Flash".into(),
        api: "openai-completions".into(),
        provider: "huggingface".into(),
        base_url: "https://router.huggingface.co/v1".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 4096.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                supports_developer_role: Some(false),
                ..Default::default()
            },
        ))),
    }
}
