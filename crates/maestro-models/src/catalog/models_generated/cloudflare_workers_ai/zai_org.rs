// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_7() -> [(&'static str, Model); 1] {
    [("@cf/zai-org/glm-4.7-flash", cf_zai_org_glm_4_dot_7_flash())]
}
fn cf_zai_org_glm_4_dot_7_flash() -> Model {
    Model {
        id: "@cf/zai-org/glm-4.7-flash".into(),
        name: "GLM-4.7-Flash".into(),
        api: "openai-completions".into(),
        provider: "cloudflare-workers-ai".into(),
        base_url: "https://api.cloudflare.com/client/v4/accounts/{CLOUDFLARE_ACCOUNT_ID}/ai/v1"
            .into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: Some(ModelCompat::OpenAICompletions(Box::new(
            OpenAICompletionsCompat {
                send_session_affinity_headers: Some(true),
                ..Default::default()
            },
        ))),
    }
}
