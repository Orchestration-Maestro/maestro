// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 1] {
    [("gpt-5.5", gpt_5_dot_5())]
}
fn gpt_5_dot_5() -> Model {
    Model {
id: "gpt-5.5".into(),
name: "GPT-5.5".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Off, None),(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 5.0,
output: 30.0,
cache_read: 0.5,
cache_write: 0.0,
},
context_window: 1_050_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}
