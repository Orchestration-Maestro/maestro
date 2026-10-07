// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 10] {
    [
        ("gpt-4", gpt_4()),
        ("gpt-4-turbo", gpt_4_turbo()),
        ("gpt-4o", gpt_4o()),
        ("gpt-4o-mini", gpt_4o_mini()),
        ("gpt-5.1", gpt_5_dot_1()),
        ("gpt-5.1-codex", gpt_5_dot_1_codex()),
        ("gpt-5.2", gpt_5_dot_2()),
        ("gpt-5.2-codex", gpt_5_dot_2_codex()),
        ("gpt-5.3-codex", gpt_5_dot_3_codex()),
        ("gpt-5.4", gpt_5_dot_4()),
    ]
}
fn gpt_4() -> Model {
    Model {
id: "gpt-4".into(),
name: "GPT-4".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text],
cost: ModelCost {
input: 30.0,
output: 60.0,
cache_read: 0.0,
cache_write: 0.0,
},
context_window: 8_192.0,
max_tokens: 8_192.0,
headers: None,
compat: None,
}
}

fn gpt_4_turbo() -> Model {
    Model {
id: "gpt-4-turbo".into(),
name: "GPT-4 Turbo".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 10.0,
output: 30.0,
cache_read: 0.0,
cache_write: 0.0,
},
context_window: 128_000.0,
max_tokens: 4_096.0,
headers: None,
compat: None,
}
}

fn gpt_4o() -> Model {
    Model {
id: "gpt-4o".into(),
name: "GPT-4o".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 2.5,
output: 10.0,
cache_read: 1.25,
cache_write: 0.0,
},
context_window: 128_000.0,
max_tokens: 16_384.0,
headers: None,
compat: None,
}
}

fn gpt_4o_mini() -> Model {
    Model {
id: "gpt-4o-mini".into(),
name: "GPT-4o mini".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: false,
thinking_level_map: None,
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 0.15,
output: 0.6,
cache_read: 0.08,
cache_write: 0.0,
},
context_window: 128_000.0,
max_tokens: 16_384.0,
headers: None,
compat: None,
}
}

fn gpt_5_dot_1() -> Model {
    Model {
id: "gpt-5.1".into(),
name: "GPT-5.1".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 1.25,
output: 10.0,
cache_read: 0.13,
cache_write: 0.0,
},
context_window: 400_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}

fn gpt_5_dot_1_codex() -> Model {
    Model {
id: "gpt-5.1-codex".into(),
name: "GPT-5.1 Codex".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Off, None)].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 1.25,
output: 10.0,
cache_read: 0.125,
cache_write: 0.0,
},
context_window: 400_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}

fn gpt_5_dot_2() -> Model {
    Model {
id: "gpt-5.2".into(),
name: "GPT-5.2".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Off, None),(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 1.75,
output: 14.0,
cache_read: 0.175,
cache_write: 0.0,
},
context_window: 400_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}

fn gpt_5_dot_2_codex() -> Model {
    Model {
id: "gpt-5.2-codex".into(),
name: "GPT-5.2 Codex".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Off, None),(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 1.75,
output: 14.0,
cache_read: 0.175,
cache_write: 0.0,
},
context_window: 400_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}

fn gpt_5_dot_3_codex() -> Model {
    Model {
id: "gpt-5.3-codex".into(),
name: "GPT-5.3 Codex".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Off, None),(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 1.75,
output: 14.0,
cache_read: 0.175,
cache_write: 0.0,
},
context_window: 400_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}

fn gpt_5_dot_4() -> Model {
    Model {
id: "gpt-5.4".into(),
name: "GPT-5.4".into(),
api: "openai-responses".into(),
provider: "cloudflare-ai-gateway".into(),
base_url: "https://gateway.ai.cloudflare.com/v1/{CLOUDFLARE_ACCOUNT_ID}/{CLOUDFLARE_GATEWAY_ID}/openai".into(),
reasoning: true,
thinking_level_map: Some([(ModelThinkingLevel::Off, None),(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
input: vec![ModelInput::Text,ModelInput::Image],
cost: ModelCost {
input: 2.5,
output: 15.0,
cache_read: 0.25,
cache_write: 0.0,
},
context_window: 1_050_000.0,
max_tokens: 128_000.0,
headers: None,
compat: None,
}
}
