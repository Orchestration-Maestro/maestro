// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("claude-3-5-haiku", claude_3_5_haiku()),
        ("claude-3-haiku", claude_3_haiku()),
        ("claude-3-opus", claude_3_opus()),
        ("claude-3-sonnet", claude_3_sonnet()),
        ("claude-3.5-haiku", claude_3_dot_5_haiku()),
        ("claude-3.5-sonnet", claude_3_dot_5_sonnet()),
        ("claude-haiku-4-5", claude_haiku_4_5()),
        ("claude-opus-4", claude_opus_4()),
        ("claude-opus-4-1", claude_opus_4_1()),
        ("claude-opus-4-5", claude_opus_4_5()),
    ]
}
fn claude_3_5_haiku() -> Model {
    Model {
id: "claude-3-5-haiku".into(),
name: "Claude Haiku 3.5 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 0.8,
output: 4.0,
cache_read: 0.08,
cache_write: 1.0,
},
context_window: 200_000.0,
max_tokens: 8_192.0,
headers: None,
compat: None,
}
}

fn claude_3_haiku() -> Model {
    Model {
id: "claude-3-haiku".into(),
name: "Claude Haiku 3".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 0.25,
output: 1.25,
cache_read: 0.03,
cache_write: 0.3,
},
context_window: 200_000.0,
max_tokens: 4_096.0,
headers: None,
compat: None,
}
}

fn claude_3_opus() -> Model {
    Model {
id: "claude-3-opus".into(),
name: "Claude Opus 3".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 15.0,
output: 75.0,
cache_read: 1.5,
cache_write: 18.75,
},
context_window: 200_000.0,
max_tokens: 4_096.0,
headers: None,
compat: None,
}
}

fn claude_3_sonnet() -> Model {
    Model {
id: "claude-3-sonnet".into(),
name: "Claude Sonnet 3".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 3.0,
output: 15.0,
cache_read: 0.3,
cache_write: 0.3,
},
context_window: 200_000.0,
max_tokens: 4_096.0,
headers: None,
compat: None,
}
}

fn claude_3_dot_5_haiku() -> Model {
    Model {
id: "claude-3.5-haiku".into(),
name: "Claude Haiku 3.5 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 0.8,
output: 4.0,
cache_read: 0.08,
cache_write: 1.0,
},
context_window: 200_000.0,
max_tokens: 8_192.0,
headers: None,
compat: None,
}
}

fn claude_3_dot_5_sonnet() -> Model {
    Model {
id: "claude-3.5-sonnet".into(),
name: "Claude Sonnet 3.5 v2".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 3.0,
output: 15.0,
cache_read: 0.3,
cache_write: 3.75,
},
context_window: 200_000.0,
max_tokens: 8_192.0,
headers: None,
compat: None,
}
}

fn claude_haiku_4_5() -> Model {
    Model {
id: "claude-haiku-4-5".into(),
name: "Claude Haiku 4.5 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 1.0,
output: 5.0,
cache_read: 0.1,
cache_write: 1.25,
},
context_window: 200_000.0,
max_tokens: 64_000.0,
headers: None,
compat: None,
}
}

fn claude_opus_4() -> Model {
    Model {
id: "claude-opus-4".into(),
name: "Claude Opus 4 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 15.0,
output: 75.0,
cache_read: 1.5,
cache_write: 18.75,
},
context_window: 200_000.0,
max_tokens: 32_000.0,
headers: None,
compat: None,
}
}

fn claude_opus_4_1() -> Model {
    Model {
id: "claude-opus-4-1".into(),
name: "Claude Opus 4.1 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 15.0,
output: 75.0,
cache_read: 1.5,
cache_write: 18.75,
},
context_window: 200_000.0,
max_tokens: 32_000.0,
headers: None,
compat: None,
}
}

fn claude_opus_4_5() -> Model {
    Model {
id: "claude-opus-4-5".into(),
name: "Claude Opus 4.5 (latest)".into(),
api: "anthropic-messages".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/anthropic".into(),
reasoning: true,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 5.0,
output: 25.0,
cache_read: 0.5,
cache_write: 6.25,
},
context_window: 200_000.0,
max_tokens: 64_000.0,
headers: None,
compat: None,
}
}
