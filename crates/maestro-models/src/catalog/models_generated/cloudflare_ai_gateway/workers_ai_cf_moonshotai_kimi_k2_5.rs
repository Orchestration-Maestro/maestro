// Generated model descriptor data.
use crate::{Model, ModelCompat, ModelCost, ModelInput, OpenAICompletionsCompat};
pub(super) fn models() -> [(&'static str, Model); 4] {
    [
        (
            "workers-ai/@cf/moonshotai/kimi-k2.5",
            workers_ai_cf_moonshotai_kimi_k2_dot_5(),
        ),
        (
            "workers-ai/@cf/moonshotai/kimi-k2.6",
            workers_ai_cf_moonshotai_kimi_k2_dot_6(),
        ),
        (
            "workers-ai/@cf/nvidia/nemotron-3-120b-a12b",
            workers_ai_cf_nvidia_nemotron_3_120b_a12b(),
        ),
        (
            "workers-ai/@cf/zai-org/glm-4.7-flash",
            workers_ai_cf_zai_org_glm_4_dot_7_flash(),
        ),
    ]
}
fn workers_ai_cf_moonshotai_kimi_k2_dot_5() -> Model {
    Model {
id: "workers-ai/@cf/moonshotai/kimi-k2.5".into(),
name: "Kimi K2.5".into(),
api: "openai-completions".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 0.6,
output: 3.0,
cache_read: 0.1,
cache_write: 0.0,
},
context_window: 256_000.0,
max_tokens: 256_000.0,
headers: None,
compat: Some(ModelCompat::OpenAICompletions(Box::new(OpenAICompletionsCompat {send_session_affinity_headers: Some(true),..Default::default()}))),
}
}

fn workers_ai_cf_moonshotai_kimi_k2_dot_6() -> Model {
    Model {
id: "workers-ai/@cf/moonshotai/kimi-k2.6".into(),
name: "Kimi K2.6".into(),
api: "openai-completions".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 0.95,
output: 4.0,
cache_read: 0.16,
cache_write: 0.0,
},
context_window: 256_000.0,
max_tokens: 256_000.0,
headers: None,
compat: Some(ModelCompat::OpenAICompletions(Box::new(OpenAICompletionsCompat {send_session_affinity_headers: Some(true),..Default::default()}))),
}
}

fn workers_ai_cf_nvidia_nemotron_3_120b_a12b() -> Model {
    Model {
id: "workers-ai/@cf/nvidia/nemotron-3-120b-a12b".into(),
name: "Nemotron 3 Super 120B".into(),
api: "openai-completions".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/compat".into(),
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
compat: Some(ModelCompat::OpenAICompletions(Box::new(OpenAICompletionsCompat {send_session_affinity_headers: Some(true),..Default::default()}))),
}
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
