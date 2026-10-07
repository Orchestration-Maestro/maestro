// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models_34() -> [(&'static str, Model); 1] {
    [(
        "workers-ai/@cf/zai-org/glm-4.7-flash",
        workers_ai_cf_zai_org_glm_4_dot_7_flash(),
    )]
}
fn workers_ai_cf_zai_org_glm_4_dot_7_flash() -> Model {
    Model {
id: "workers-ai/@cf/zai-org/glm-4.7-flash".into(),
name: "GLM-4.7-Flash".into(),
api: "openai-completions".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat".into(),
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
compat: Some(ModelCompat::OpenAICompletions(Box::new(OpenAICompletionsCompat {send_session_affinity_headers: Some(true),..Default::default()}))),
}
}
