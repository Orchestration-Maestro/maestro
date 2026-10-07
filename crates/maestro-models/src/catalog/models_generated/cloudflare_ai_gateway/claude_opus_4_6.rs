// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 5] {
    [
        ("claude-opus-4-6", claude_opus_4_6()),
        ("claude-opus-4-7", claude_opus_4_7()),
        ("claude-sonnet-4", claude_sonnet_4()),
        ("claude-sonnet-4-5", claude_sonnet_4_5()),
        ("claude-sonnet-4-6", claude_sonnet_4_6()),
    ]
}
fn claude_opus_4_6() -> Model {
    Model {
id: "claude-opus-4-6".into(),
name: "Claude Opus 4.6 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 5.0,
output: 25.0,
cache_read: 0.5,
cache_write: 6.25,
},
context_window: 1_000_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}

fn claude_opus_4_7() -> Model {
    Model {
id: "claude-opus-4-7".into(),
name: "Claude Opus 4.7".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 5.0,
output: 25.0,
cache_read: 0.5,
cache_write: 6.25,
},
context_window: 1_000_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}

fn claude_sonnet_4() -> Model {
    Model {
id: "claude-sonnet-4".into(),
name: "Claude Sonnet 4 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 3.0,
output: 15.0,
cache_read: 0.3,
cache_write: 3.75,
},
context_window: 200_000.0,
max_tokens: 64_000.0,
headers: None,
compat: None,
}
}

fn claude_sonnet_4_5() -> Model {
    Model {
id: "claude-sonnet-4-5".into(),
name: "Claude Sonnet 4.5 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 3.0,
output: 15.0,
cache_read: 0.3,
cache_write: 3.75,
},
context_window: 200_000.0,
max_tokens: 64_000.0,
headers: None,
compat: None,
}
}

fn claude_sonnet_4_6() -> Model {
    Model {
id: "claude-sonnet-4-6".into(),
name: "Claude Sonnet 4.6".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 3.0,
output: 15.0,
cache_read: 0.3,
cache_write: 3.75,
},
context_window: 1_000_000.0,
max_tokens: 64_000.0,
headers: None,
compat: None,
}
}
