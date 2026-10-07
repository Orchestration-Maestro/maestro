// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 5] {
    [
        ("o1", o1()),
        ("o3", o3()),
        ("o3-mini", o3_mini()),
        ("o3-pro", o3_pro()),
        ("o4-mini", o4_mini()),
    ]
}
fn o1() -> Model {
    Model {
id: "o1".into(),
name: "o1".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 15.0,
output: 60.0,
cache_read: 7.5,
cache_write: 0.0,
},
context_window: 200_000.0,
max_tokens: 100_000.0,
headers: None,
compat: None,
}
}

fn o3() -> Model {
    Model {
id: "o3".into(),
name: "o3".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 2.0,
output: 8.0,
cache_read: 0.5,
cache_write: 0.0,
},
context_window: 200_000.0,
max_tokens: 100_000.0,
headers: None,
compat: None,
}
}

fn o3_mini() -> Model {
    Model {
id: "o3-mini".into(),
name: "o3-mini".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text],
cost: ModelCost {
input: 1.1,
output: 4.4,
cache_read: 0.55,
cache_write: 0.0,
},
context_window: 200_000.0,
max_tokens: 100_000.0,
headers: None,
compat: None,
}
}

fn o3_pro() -> Model {
    Model {
id: "o3-pro".into(),
name: "o3-pro".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 20.0,
output: 80.0,
cache_read: 0.0,
cache_write: 0.0,
},
context_window: 200_000.0,
max_tokens: 100_000.0,
headers: None,
compat: None,
}
}

fn o4_mini() -> Model {
    Model {
id: "o4-mini".into(),
name: "o4-mini".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 1.1,
output: 4.4,
cache_read: 0.28,
cache_write: 0.0,
},
context_window: 200_000.0,
max_tokens: 100_000.0,
headers: None,
compat: None,
}
}
