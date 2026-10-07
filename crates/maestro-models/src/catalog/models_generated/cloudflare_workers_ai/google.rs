// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_0() -> [(&'static str, Model); 1] {
    [(
        "@cf/google/gemma-4-26b-a4b-it",
        cf_google_gemma_4_26b_a4b_it(),
    )]
}
fn cf_google_gemma_4_26b_a4b_it() -> Model {
    Model {
        id: "@cf/google/gemma-4-26b-a4b-it".into(),
        name: "Gemma 4 26B A4B IT".into(),
        api: "openai-completions".into(),
        provider: "cloudflare-workers-ai".into(),
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1"
            .into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.3,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
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
