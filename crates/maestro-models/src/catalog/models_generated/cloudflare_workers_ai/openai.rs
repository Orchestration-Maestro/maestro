// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_5() -> [(&'static str, Model); 2] {
    [
        ("@cf/openai/gpt-oss-120b", cf_openai_gpt_oss_120b()),
        ("@cf/openai/gpt-oss-20b", cf_openai_gpt_oss_20b()),
    ]
}
fn cf_openai_gpt_oss_120b() -> Model {
    Model {
        id: "@cf/openai/gpt-oss-120b".into(),
        name: "GPT OSS 120B".into(),
        api: "openai-completions".into(),
        provider: "cloudflare-workers-ai".into(),
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1"
            .into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.35,
            output: 0.75,
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

fn cf_openai_gpt_oss_20b() -> Model {
    Model {
        id: "@cf/openai/gpt-oss-20b".into(),
        name: "GPT OSS 20B".into(),
        api: "openai-completions".into(),
        provider: "cloudflare-workers-ai".into(),
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1"
            .into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
            output: 0.3,
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
