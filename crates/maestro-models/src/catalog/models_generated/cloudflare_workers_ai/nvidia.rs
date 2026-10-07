// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_4() -> [(&'static str, Model); 1] {
    [(
        "@cf/nvidia/nemotron-3-120b-a12b",
        cf_nvidia_nemotron_3_120b_a12b(),
    )]
}
fn cf_nvidia_nemotron_3_120b_a12b() -> Model {
    Model {
        id: "@cf/nvidia/nemotron-3-120b-a12b".into(),
        name: "Nemotron 3 Super 120B".into(),
        api: "openai-completions".into(),
        provider: "cloudflare-workers-ai".into(),
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1"
            .into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                send_session_affinity_headers: Some(true),
                ..Default::default()
            },
        ))),
    }
}
