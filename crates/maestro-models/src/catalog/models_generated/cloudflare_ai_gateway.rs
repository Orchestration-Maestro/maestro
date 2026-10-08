//! Generated model descriptors for cloudflare-ai-gateway.

use crate::{
    Model, ModelCompat, ModelCost, ModelInput, ModelThinkingLevel, OpenAICompletionsCompat,
};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_10());
    models.extend(models_15());
    models.extend(models_25());
    models.extend(models_26());
    models.extend(models_31());
    models.extend(models_33());
    models.extend(models_34());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 10] {
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
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_10() -> [(&'static str, Model); 5] {
    [
        ("claude-opus-4-6", claude_opus_4_6()),
        ("claude-opus-4-7", claude_opus_4_7()),
        ("claude-sonnet-4", claude_sonnet_4()),
        ("claude-sonnet-4-5", claude_sonnet_4_5()),
        ("claude-sonnet-4-6", claude_sonnet_4_6()),
    ]
}
/// Construct the recorded descriptor for this model.
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
max_tokens: 8192.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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
max_tokens: 4096.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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
max_tokens: 4096.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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
max_tokens: 4096.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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
max_tokens: 8192.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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
max_tokens: 8192.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_15() -> [(&'static str, Model); 10] {
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
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_25() -> [(&'static str, Model); 1] {
    [("gpt-5.5", gpt_5_dot_5())]
}
/// Construct the recorded descriptor for this model.
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
context_window: 8192.0,
max_tokens: 8192.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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
max_tokens: 4096.0,
headers: None,
compat: None,
}
}

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_31() -> [(&'static str, Model); 2] {
    [
        (
            "workers-ai/@cf/moonshotai/kimi-k2.5",
            workers_ai_cf_moonshotai_kimi_k2_dot_5(),
        ),
        (
            "workers-ai/@cf/moonshotai/kimi-k2.6",
            workers_ai_cf_moonshotai_kimi_k2_dot_6(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_33() -> [(&'static str, Model); 1] {
    [(
        "workers-ai/@cf/nvidia/nemotron-3-120b-a12b",
        workers_ai_cf_nvidia_nemotron_3_120b_a12b(),
    )]
}
/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_26() -> [(&'static str, Model); 5] {
    [
        ("o1", o1()),
        ("o3", o3()),
        ("o3-mini", o3_mini()),
        ("o3-pro", o3_pro()),
        ("o4-mini", o4_mini()),
    ]
}
/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Construct the recorded descriptor for this model.
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_34() -> [(&'static str, Model); 1] {
    [(
        "workers-ai/@cf/zai-org/glm-4.7-flash",
        workers_ai_cf_zai_org_glm_4_dot_7_flash(),
    )]
}
/// Construct the recorded descriptor for this model.
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
