// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_1() -> [(&'static str, Model); 1] {
    [(
        "@cf/meta/llama-4-scout-17b-16e-instruct",
        cf_meta_llama_4_scout_17b_16e_instruct(),
    )]
}
fn cf_meta_llama_4_scout_17b_16e_instruct() -> Model {
    Model {
        id: "@cf/meta/llama-4-scout-17b-16e-instruct".into(),
        name: "Llama 4 Scout 17B 16E Instruct".into(),
        api: "openai-completions".into(),
        provider: "cloudflare-workers-ai".into(),
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1"
            .into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.27,
            output: 0.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                send_session_affinity_headers: Some(true),
                ..Default::default()
            },
        ))),
    }
}
