//! Generated model descriptors for amazon-bedrock.

use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
use indexmap::IndexMap;

/// Assemble this provider's descriptors in registry order.
pub(super) fn models() -> IndexMap<&'static str, Model> {
    let mut models = IndexMap::new();
    models.extend(models_0());
    models.extend(models_5());
    models.extend(models_15());
    models.extend(models_21());
    models.extend(models_24());
    models.extend(models_26());
    models.extend(models_36());
    models.extend(models_38());
    models.extend(models_40());
    models.extend(models_50());
    models.extend(models_53());
    models.extend(models_62());
    models.extend(models_63());
    models.extend(models_64());
    models.extend(models_68());
    models.extend(models_72());
    models.extend(models_79());
    models.extend(models_85());
    models.extend(models_88());
    models.extend(models_90());
    models
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_0() -> [(&'static str, Model); 5] {
    [
        ("amazon.nova-2-lite-v1:0", amazon_dot_nova_2_lite_v1_0()),
        ("amazon.nova-lite-v1:0", amazon_dot_nova_lite_v1_0()),
        ("amazon.nova-micro-v1:0", amazon_dot_nova_micro_v1_0()),
        ("amazon.nova-premier-v1:0", amazon_dot_nova_premier_v1_0()),
        ("amazon.nova-pro-v1:0", amazon_dot_nova_pro_v1_0()),
    ]
}
/// Construct the recorded descriptor for this model.
fn amazon_dot_nova_2_lite_v1_0() -> Model {
    Model {
        id: "amazon.nova-2-lite-v1:0".into(),
        name: "Nova 2 Lite".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.33,
            output: 2.75,
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
fn amazon_dot_nova_lite_v1_0() -> Model {
    Model {
        id: "amazon.nova-lite-v1:0".into(),
        name: "Nova Lite".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
            cache_read: 0.015,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn amazon_dot_nova_micro_v1_0() -> Model {
    Model {
        id: "amazon.nova-micro-v1:0".into(),
        name: "Nova Micro".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.035,
            output: 0.14,
            cache_read: 0.008_75,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn amazon_dot_nova_premier_v1_0() -> Model {
    Model {
        id: "amazon.nova-premier-v1:0".into(),
        name: "Nova Premier".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.5,
            output: 12.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn amazon_dot_nova_pro_v1_0() -> Model {
    Model {
        id: "amazon.nova-pro-v1:0".into(),
        name: "Nova Pro".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.8,
            output: 3.2,
            cache_read: 0.2,
            cache_write: 0.0,
        },
        context_window: 300_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_5() -> [(&'static str, Model); 10] {
    [
        (
            "anthropic.claude-3-5-haiku-20241022-v1:0",
            anthropic_dot_claude_3_5_haiku_20241022_v1_0(),
        ),
        (
            "anthropic.claude-3-5-sonnet-20240620-v1:0",
            anthropic_dot_claude_3_5_sonnet_20240620_v1_0(),
        ),
        (
            "anthropic.claude-3-5-sonnet-20241022-v2:0",
            anthropic_dot_claude_3_5_sonnet_20241022_v2_0(),
        ),
        (
            "anthropic.claude-3-7-sonnet-20250219-v1:0",
            anthropic_dot_claude_3_7_sonnet_20250219_v1_0(),
        ),
        (
            "anthropic.claude-3-haiku-20240307-v1:0",
            anthropic_dot_claude_3_haiku_20240307_v1_0(),
        ),
        (
            "anthropic.claude-haiku-4-5-20251001-v1:0",
            anthropic_dot_claude_haiku_4_5_20251001_v1_0(),
        ),
        (
            "anthropic.claude-opus-4-1-20250805-v1:0",
            anthropic_dot_claude_opus_4_1_20250805_v1_0(),
        ),
        (
            "anthropic.claude-opus-4-20250514-v1:0",
            anthropic_dot_claude_opus_4_20250514_v1_0(),
        ),
        (
            "anthropic.claude-opus-4-5-20251101-v1:0",
            anthropic_dot_claude_opus_4_5_20251101_v1_0(),
        ),
        (
            "anthropic.claude-opus-4-6-v1",
            anthropic_dot_claude_opus_4_6_v1(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_15() -> [(&'static str, Model); 6] {
    [
        ("anthropic.claude-opus-4-7", anthropic_dot_claude_opus_4_7()),
        (
            "anthropic.claude-sonnet-4-20250514-v1:0",
            anthropic_dot_claude_sonnet_4_20250514_v1_0(),
        ),
        (
            "anthropic.claude-sonnet-4-5-20250929-v1:0",
            anthropic_dot_claude_sonnet_4_5_20250929_v1_0(),
        ),
        (
            "anthropic.claude-sonnet-4-6",
            anthropic_dot_claude_sonnet_4_6(),
        ),
        (
            "au.anthropic.claude-opus-4-6-v1",
            au_dot_anthropic_dot_claude_opus_4_6_v1(),
        ),
        (
            "au.anthropic.claude-sonnet-4-6",
            au_dot_anthropic_dot_claude_sonnet_4_6(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_24() -> [(&'static str, Model); 2] {
    [
        (
            "eu.anthropic.claude-haiku-4-5-20251001-v1:0",
            eu_dot_anthropic_dot_claude_haiku_4_5_20251001_v1_0(),
        ),
        (
            "eu.anthropic.claude-opus-4-5-20251101-v1:0",
            eu_dot_anthropic_dot_claude_opus_4_5_20251101_v1_0(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn anthropic_dot_claude_3_5_haiku_20241022_v1_0() -> Model {
    Model {
        id: "anthropic.claude-3-5-haiku-20241022-v1:0".into(),
        name: "Claude Haiku 3.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_3_5_sonnet_20240620_v1_0() -> Model {
    Model {
        id: "anthropic.claude-3-5-sonnet-20240620-v1:0".into(),
        name: "Claude Sonnet 3.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_3_5_sonnet_20241022_v2_0() -> Model {
    Model {
        id: "anthropic.claude-3-5-sonnet-20241022-v2:0".into(),
        name: "Claude Sonnet 3.5 v2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_3_7_sonnet_20250219_v1_0() -> Model {
    Model {
        id: "anthropic.claude-3-7-sonnet-20250219-v1:0".into(),
        name: "Claude Sonnet 3.7".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_3_haiku_20240307_v1_0() -> Model {
    Model {
        id: "anthropic.claude-3-haiku-20240307-v1:0".into(),
        name: "Claude Haiku 3".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.25,
            output: 1.25,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn anthropic_dot_claude_haiku_4_5_20251001_v1_0() -> Model {
    Model {
        id: "anthropic.claude-haiku-4-5-20251001-v1:0".into(),
        name: "Claude Haiku 4.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_opus_4_1_20250805_v1_0() -> Model {
    Model {
        id: "anthropic.claude-opus-4-1-20250805-v1:0".into(),
        name: "Claude Opus 4.1".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_opus_4_20250514_v1_0() -> Model {
    Model {
        id: "anthropic.claude-opus-4-20250514-v1:0".into(),
        name: "Claude Opus 4".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_opus_4_5_20251101_v1_0() -> Model {
    Model {
        id: "anthropic.claude-opus-4-5-20251101-v1:0".into(),
        name: "Claude Opus 4.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_opus_4_6_v1() -> Model {
    Model {
        id: "anthropic.claude-opus-4-6-v1".into(),
        name: "Claude Opus 4.6".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_opus_4_7() -> Model {
    Model {
        id: "anthropic.claude-opus-4-7".into(),
        name: "Claude Opus 4.7".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_sonnet_4_20250514_v1_0() -> Model {
    Model {
        id: "anthropic.claude-sonnet-4-20250514-v1:0".into(),
        name: "Claude Sonnet 4".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_sonnet_4_5_20250929_v1_0() -> Model {
    Model {
        id: "anthropic.claude-sonnet-4-5-20250929-v1:0".into(),
        name: "Claude Sonnet 4.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn anthropic_dot_claude_sonnet_4_6() -> Model {
    Model {
        id: "anthropic.claude-sonnet-4-6".into(),
        name: "Claude Sonnet 4.6".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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

/// Construct the recorded descriptor for this model.
fn au_dot_anthropic_dot_claude_opus_4_6_v1() -> Model {
    Model {
        id: "au.anthropic.claude-opus-4-6-v1".into(),
        name: "AU Anthropic Claude Opus 4.6".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 16.5,
            output: 82.5,
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
fn au_dot_anthropic_dot_claude_sonnet_4_6() -> Model {
    Model {
        id: "au.anthropic.claude-sonnet-4-6".into(),
        name: "AU Anthropic Claude Sonnet 4.6".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 3.3,
            output: 16.5,
            cache_read: 0.33,
            cache_write: 4.125,
        },
        context_window: 1_000_000.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn eu_dot_anthropic_dot_claude_haiku_4_5_20251001_v1_0() -> Model {
    Model {
        id: "eu.anthropic.claude-haiku-4-5-20251001-v1:0".into(),
        name: "Claude Haiku 4.5 (EU)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.eu-central-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn eu_dot_anthropic_dot_claude_opus_4_5_20251101_v1_0() -> Model {
    Model {
        id: "eu.anthropic.claude-opus-4-5-20251101-v1:0".into(),
        name: "Claude Opus 4.5 (EU)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.eu-central-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_26() -> [(&'static str, Model); 10] {
    [
        (
            "eu.anthropic.claude-opus-4-6-v1",
            eu_dot_anthropic_dot_claude_opus_4_6_v1(),
        ),
        (
            "eu.anthropic.claude-opus-4-7",
            eu_dot_anthropic_dot_claude_opus_4_7(),
        ),
        (
            "eu.anthropic.claude-sonnet-4-20250514-v1:0",
            eu_dot_anthropic_dot_claude_sonnet_4_20250514_v1_0(),
        ),
        (
            "eu.anthropic.claude-sonnet-4-5-20250929-v1:0",
            eu_dot_anthropic_dot_claude_sonnet_4_5_20250929_v1_0(),
        ),
        (
            "eu.anthropic.claude-sonnet-4-6",
            eu_dot_anthropic_dot_claude_sonnet_4_6(),
        ),
        (
            "global.anthropic.claude-haiku-4-5-20251001-v1:0",
            global_dot_anthropic_dot_claude_haiku_4_5_20251001_v1_0(),
        ),
        (
            "global.anthropic.claude-opus-4-5-20251101-v1:0",
            global_dot_anthropic_dot_claude_opus_4_5_20251101_v1_0(),
        ),
        (
            "global.anthropic.claude-opus-4-6-v1",
            global_dot_anthropic_dot_claude_opus_4_6_v1(),
        ),
        (
            "global.anthropic.claude-opus-4-7",
            global_dot_anthropic_dot_claude_opus_4_7(),
        ),
        (
            "global.anthropic.claude-sonnet-4-20250514-v1:0",
            global_dot_anthropic_dot_claude_sonnet_4_20250514_v1_0(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_36() -> [(&'static str, Model); 2] {
    [
        (
            "global.anthropic.claude-sonnet-4-5-20250929-v1:0",
            global_dot_anthropic_dot_claude_sonnet_4_5_20250929_v1_0(),
        ),
        (
            "global.anthropic.claude-sonnet-4-6",
            global_dot_anthropic_dot_claude_sonnet_4_6(),
        ),
    ]
}
/// Assemble a batch of descriptors in registry order.
pub(super) fn models_79() -> [(&'static str, Model); 6] {
    [
        (
            "us.anthropic.claude-haiku-4-5-20251001-v1:0",
            us_dot_anthropic_dot_claude_haiku_4_5_20251001_v1_0(),
        ),
        (
            "us.anthropic.claude-opus-4-1-20250805-v1:0",
            us_dot_anthropic_dot_claude_opus_4_1_20250805_v1_0(),
        ),
        (
            "us.anthropic.claude-opus-4-20250514-v1:0",
            us_dot_anthropic_dot_claude_opus_4_20250514_v1_0(),
        ),
        (
            "us.anthropic.claude-opus-4-5-20251101-v1:0",
            us_dot_anthropic_dot_claude_opus_4_5_20251101_v1_0(),
        ),
        (
            "us.anthropic.claude-opus-4-6-v1",
            us_dot_anthropic_dot_claude_opus_4_6_v1(),
        ),
        (
            "us.anthropic.claude-opus-4-7",
            us_dot_anthropic_dot_claude_opus_4_7(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn eu_dot_anthropic_dot_claude_opus_4_6_v1() -> Model {
    Model {
        id: "eu.anthropic.claude-opus-4-6-v1".into(),
        name: "Claude Opus 4.6 (EU)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.eu-central-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn eu_dot_anthropic_dot_claude_opus_4_7() -> Model {
    Model {
        id: "eu.anthropic.claude-opus-4-7".into(),
        name: "Claude Opus 4.7 (EU)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.eu-central-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn eu_dot_anthropic_dot_claude_sonnet_4_20250514_v1_0() -> Model {
    Model {
        id: "eu.anthropic.claude-sonnet-4-20250514-v1:0".into(),
        name: "Claude Sonnet 4 (EU)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.eu-central-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn eu_dot_anthropic_dot_claude_sonnet_4_5_20250929_v1_0() -> Model {
    Model {
        id: "eu.anthropic.claude-sonnet-4-5-20250929-v1:0".into(),
        name: "Claude Sonnet 4.5 (EU)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.eu-central-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn eu_dot_anthropic_dot_claude_sonnet_4_6() -> Model {
    Model {
        id: "eu.anthropic.claude-sonnet-4-6".into(),
        name: "Claude Sonnet 4.6 (EU)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.eu-central-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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

/// Construct the recorded descriptor for this model.
fn global_dot_anthropic_dot_claude_haiku_4_5_20251001_v1_0() -> Model {
    Model {
        id: "global.anthropic.claude-haiku-4-5-20251001-v1:0".into(),
        name: "Claude Haiku 4.5 (Global)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn global_dot_anthropic_dot_claude_opus_4_5_20251101_v1_0() -> Model {
    Model {
        id: "global.anthropic.claude-opus-4-5-20251101-v1:0".into(),
        name: "Claude Opus 4.5 (Global)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn global_dot_anthropic_dot_claude_opus_4_6_v1() -> Model {
    Model {
        id: "global.anthropic.claude-opus-4-6-v1".into(),
        name: "Claude Opus 4.6 (Global)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn global_dot_anthropic_dot_claude_opus_4_7() -> Model {
    Model {
        id: "global.anthropic.claude-opus-4-7".into(),
        name: "Claude Opus 4.7 (Global)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn global_dot_anthropic_dot_claude_sonnet_4_20250514_v1_0() -> Model {
    Model {
        id: "global.anthropic.claude-sonnet-4-20250514-v1:0".into(),
        name: "Claude Sonnet 4 (Global)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn global_dot_anthropic_dot_claude_sonnet_4_5_20250929_v1_0() -> Model {
    Model {
        id: "global.anthropic.claude-sonnet-4-5-20250929-v1:0".into(),
        name: "Claude Sonnet 4.5 (Global)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn global_dot_anthropic_dot_claude_sonnet_4_6() -> Model {
    Model {
        id: "global.anthropic.claude-sonnet-4-6".into(),
        name: "Claude Sonnet 4.6 (Global)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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

/// Construct the recorded descriptor for this model.
fn us_dot_anthropic_dot_claude_haiku_4_5_20251001_v1_0() -> Model {
    Model {
        id: "us.anthropic.claude-haiku-4-5-20251001-v1:0".into(),
        name: "Claude Haiku 4.5 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn us_dot_anthropic_dot_claude_opus_4_1_20250805_v1_0() -> Model {
    Model {
        id: "us.anthropic.claude-opus-4-1-20250805-v1:0".into(),
        name: "Claude Opus 4.1 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn us_dot_anthropic_dot_claude_opus_4_20250514_v1_0() -> Model {
    Model {
        id: "us.anthropic.claude-opus-4-20250514-v1:0".into(),
        name: "Claude Opus 4 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn us_dot_anthropic_dot_claude_opus_4_5_20251101_v1_0() -> Model {
    Model {
        id: "us.anthropic.claude-opus-4-5-20251101-v1:0".into(),
        name: "Claude Opus 4.5 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn us_dot_anthropic_dot_claude_opus_4_6_v1() -> Model {
    Model {
        id: "us.anthropic.claude-opus-4-6-v1".into(),
        name: "Claude Opus 4.6 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("max".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn us_dot_anthropic_dot_claude_opus_4_7() -> Model {
    Model {
        id: "us.anthropic.claude-opus-4-7".into(),
        name: "Claude Opus 4.7 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: Some([(ModelThinkingLevel::Xhigh, Some("xhigh".into()))].into()),
        input: vec![ModelInput::Text, ModelInput::Image],
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

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_85() -> [(&'static str, Model); 3] {
    [
        (
            "us.anthropic.claude-sonnet-4-20250514-v1:0",
            us_dot_anthropic_dot_claude_sonnet_4_20250514_v1_0(),
        ),
        (
            "us.anthropic.claude-sonnet-4-5-20250929-v1:0",
            us_dot_anthropic_dot_claude_sonnet_4_5_20250929_v1_0(),
        ),
        (
            "us.anthropic.claude-sonnet-4-6",
            us_dot_anthropic_dot_claude_sonnet_4_6(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn us_dot_anthropic_dot_claude_sonnet_4_20250514_v1_0() -> Model {
    Model {
        id: "us.anthropic.claude-sonnet-4-20250514-v1:0".into(),
        name: "Claude Sonnet 4 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn us_dot_anthropic_dot_claude_sonnet_4_5_20250929_v1_0() -> Model {
    Model {
        id: "us.anthropic.claude-sonnet-4-5-20250929-v1:0".into(),
        name: "Claude Sonnet 4.5 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
fn us_dot_anthropic_dot_claude_sonnet_4_6() -> Model {
    Model {
        id: "us.anthropic.claude-sonnet-4-6".into(),
        name: "Claude Sonnet 4.6 (US)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
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
pub(super) fn models_21() -> [(&'static str, Model); 3] {
    [
        ("deepseek.r1-v1:0", deepseek_dot_r1_v1_0()),
        ("deepseek.v3-v1:0", deepseek_dot_v3_v1_0()),
        ("deepseek.v3.2", deepseek_dot_v3_dot_2()),
    ]
}
/// Construct the recorded descriptor for this model.
fn deepseek_dot_r1_v1_0() -> Model {
    Model {
        id: "deepseek.r1-v1:0".into(),
        name: "DeepSeek-R1".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.35,
            output: 5.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 32_768.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_dot_v3_v1_0() -> Model {
    Model {
        id: "deepseek.v3-v1:0".into(),
        name: "DeepSeek-V3.1".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.58,
            output: 1.68,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 81_920.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn deepseek_dot_v3_dot_2() -> Model {
    Model {
        id: "deepseek.v3.2".into(),
        name: "DeepSeek-V3.2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.62,
            output: 1.85,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 163_840.0,
        max_tokens: 81_920.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_38() -> [(&'static str, Model); 2] {
    [
        ("google.gemma-3-27b-it", google_dot_gemma_3_27b_it()),
        ("google.gemma-3-4b-it", google_dot_gemma_3_4b_it()),
    ]
}
/// Construct the recorded descriptor for this model.
fn google_dot_gemma_3_27b_it() -> Model {
    Model {
        id: "google.gemma-3-27b-it".into(),
        name: "Google Gemma 3 27B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.12,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn google_dot_gemma_3_4b_it() -> Model {
    Model {
        id: "google.gemma-3-4b-it".into(),
        name: "Gemma 3 4B IT".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.04,
            output: 0.08,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_40() -> [(&'static str, Model); 10] {
    [
        (
            "meta.llama3-1-405b-instruct-v1:0",
            meta_dot_llama3_1_405b_instruct_v1_0(),
        ),
        (
            "meta.llama3-1-70b-instruct-v1:0",
            meta_dot_llama3_1_70b_instruct_v1_0(),
        ),
        (
            "meta.llama3-1-8b-instruct-v1:0",
            meta_dot_llama3_1_8b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-11b-instruct-v1:0",
            meta_dot_llama3_2_11b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-1b-instruct-v1:0",
            meta_dot_llama3_2_1b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-3b-instruct-v1:0",
            meta_dot_llama3_2_3b_instruct_v1_0(),
        ),
        (
            "meta.llama3-2-90b-instruct-v1:0",
            meta_dot_llama3_2_90b_instruct_v1_0(),
        ),
        (
            "meta.llama3-3-70b-instruct-v1:0",
            meta_dot_llama3_3_70b_instruct_v1_0(),
        ),
        (
            "meta.llama4-maverick-17b-instruct-v1:0",
            meta_dot_llama4_maverick_17b_instruct_v1_0(),
        ),
        (
            "meta.llama4-scout-17b-instruct-v1:0",
            meta_dot_llama4_scout_17b_instruct_v1_0(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn meta_dot_llama3_1_405b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-1-405b-instruct-v1:0".into(),
        name: "Llama 3.1 405B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.4,
            output: 2.4,
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
fn meta_dot_llama3_1_70b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-1-70b-instruct-v1:0".into(),
        name: "Llama 3.1 70B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
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
fn meta_dot_llama3_1_8b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-1-8b-instruct-v1:0".into(),
        name: "Llama 3.1 8B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.22,
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
fn meta_dot_llama3_2_11b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-11b-instruct-v1:0".into(),
        name: "Llama 3.2 11B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.16,
            output: 0.16,
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
fn meta_dot_llama3_2_1b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-1b-instruct-v1:0".into(),
        name: "Llama 3.2 1B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.1,
            output: 0.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_dot_llama3_2_3b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-3b-instruct-v1:0".into(),
        name: "Llama 3.2 3B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_dot_llama3_2_90b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-2-90b-instruct-v1:0".into(),
        name: "Llama 3.2 90B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
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
fn meta_dot_llama3_3_70b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama3-3-70b-instruct-v1:0".into(),
        name: "Llama 3.3 70B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.72,
            output: 0.72,
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
fn meta_dot_llama4_maverick_17b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama4-maverick-17b-instruct-v1:0".into(),
        name: "Llama 4 Maverick 17B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.24,
            output: 0.97,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_000_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn meta_dot_llama4_scout_17b_instruct_v1_0() -> Model {
    Model {
        id: "meta.llama4-scout-17b-instruct-v1:0".into(),
        name: "Llama 4 Scout 17B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.17,
            output: 0.66,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 3_500_000.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_50() -> [(&'static str, Model); 3] {
    [
        ("minimax.minimax-m2", minimax_dot_minimax_m2()),
        ("minimax.minimax-m2.1", minimax_dot_minimax_m2_dot_1()),
        ("minimax.minimax-m2.5", minimax_dot_minimax_m2_dot_5()),
    ]
}
/// Construct the recorded descriptor for this model.
fn minimax_dot_minimax_m2() -> Model {
    Model {
        id: "minimax.minimax-m2".into(),
        name: "MiniMax M2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_608.0,
        max_tokens: 128_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_dot_minimax_m2_dot_1() -> Model {
    Model {
        id: "minimax.minimax-m2.1".into(),
        name: "MiniMax M2.1".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn minimax_dot_minimax_m2_dot_5() -> Model {
    Model {
        id: "minimax.minimax-m2.5".into(),
        name: "MiniMax M2.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.3,
            output: 1.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 196_608.0,
        max_tokens: 98_304.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_53() -> [(&'static str, Model); 9] {
    [
        ("mistral.devstral-2-123b", mistral_dot_devstral_2_123b()),
        (
            "mistral.magistral-small-2509",
            mistral_dot_magistral_small_2509(),
        ),
        (
            "mistral.ministral-3-14b-instruct",
            mistral_dot_ministral_3_14b_instruct(),
        ),
        (
            "mistral.ministral-3-3b-instruct",
            mistral_dot_ministral_3_3b_instruct(),
        ),
        (
            "mistral.ministral-3-8b-instruct",
            mistral_dot_ministral_3_8b_instruct(),
        ),
        (
            "mistral.mistral-large-3-675b-instruct",
            mistral_dot_mistral_large_3_675b_instruct(),
        ),
        (
            "mistral.pixtral-large-2502-v1:0",
            mistral_dot_pixtral_large_2502_v1_0(),
        ),
        (
            "mistral.voxtral-mini-3b-2507",
            mistral_dot_voxtral_mini_3b_2507(),
        ),
        (
            "mistral.voxtral-small-24b-2507",
            mistral_dot_voxtral_small_24b_2507(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn mistral_dot_devstral_2_123b() -> Model {
    Model {
        id: "mistral.devstral-2-123b".into(),
        name: "Devstral 2 123B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.4,
            output: 2.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_dot_magistral_small_2509() -> Model {
    Model {
        id: "mistral.magistral-small-2509".into(),
        name: "Magistral Small 1.2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 40_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_dot_ministral_3_14b_instruct() -> Model {
    Model {
        id: "mistral.ministral-3-14b-instruct".into(),
        name: "Ministral 14B 3.0".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.2,
            output: 0.2,
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
fn mistral_dot_ministral_3_3b_instruct() -> Model {
    Model {
        id: "mistral.ministral-3-3b-instruct".into(),
        name: "Ministral 3 3B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.1,
            output: 0.1,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_dot_ministral_3_8b_instruct() -> Model {
    Model {
        id: "mistral.ministral-3-8b-instruct".into(),
        name: "Ministral 3 8B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.15,
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
fn mistral_dot_mistral_large_3_675b_instruct() -> Model {
    Model {
        id: "mistral.mistral-large-3-675b-instruct".into(),
        name: "Mistral Large 3".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.5,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_dot_pixtral_large_2502_v1_0() -> Model {
    Model {
        id: "mistral.pixtral-large-2502-v1:0".into(),
        name: "Pixtral Large (25.02)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 2.0,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn mistral_dot_voxtral_mini_3b_2507() -> Model {
    Model {
        id: "mistral.voxtral-mini-3b-2507".into(),
        name: "Voxtral Mini 3B 2507".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.04,
            output: 0.04,
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
fn mistral_dot_voxtral_small_24b_2507() -> Model {
    Model {
        id: "mistral.voxtral-small-24b-2507".into(),
        name: "Voxtral Small 24B 2507".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.35,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 32_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_62() -> [(&'static str, Model); 1] {
    [("moonshot.kimi-k2-thinking", moonshot_dot_kimi_k2_thinking())]
}
/// Construct the recorded descriptor for this model.
fn moonshot_dot_kimi_k2_thinking() -> Model {
    Model {
        id: "moonshot.kimi-k2-thinking".into(),
        name: "Kimi K2 Thinking".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_63() -> [(&'static str, Model); 1] {
    [("moonshotai.kimi-k2.5", moonshotai_dot_kimi_k2_dot_5())]
}
/// Construct the recorded descriptor for this model.
fn moonshotai_dot_kimi_k2_dot_5() -> Model {
    Model {
        id: "moonshotai.kimi-k2.5".into(),
        name: "Kimi K2.5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.6,
            output: 3.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 256_000.0,
        max_tokens: 256_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_64() -> [(&'static str, Model); 4] {
    [
        (
            "nvidia.nemotron-nano-12b-v2",
            nvidia_dot_nemotron_nano_12b_v2(),
        ),
        (
            "nvidia.nemotron-nano-3-30b",
            nvidia_dot_nemotron_nano_3_30b(),
        ),
        (
            "nvidia.nemotron-nano-9b-v2",
            nvidia_dot_nemotron_nano_9b_v2(),
        ),
        (
            "nvidia.nemotron-super-3-120b",
            nvidia_dot_nemotron_super_3_120b(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn nvidia_dot_nemotron_nano_12b_v2() -> Model {
    Model {
        id: "nvidia.nemotron-nano-12b-v2".into(),
        name: "NVIDIA Nemotron Nano 12B v2 VL BF16".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.2,
            output: 0.6,
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
fn nvidia_dot_nemotron_nano_3_30b() -> Model {
    Model {
        id: "nvidia.nemotron-nano-3-30b".into(),
        name: "NVIDIA Nemotron Nano 3 30B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.24,
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
fn nvidia_dot_nemotron_nano_9b_v2() -> Model {
    Model {
        id: "nvidia.nemotron-nano-9b-v2".into(),
        name: "NVIDIA Nemotron Nano 9B v2".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.06,
            output: 0.23,
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
fn nvidia_dot_nemotron_super_3_120b() -> Model {
    Model {
        id: "nvidia.nemotron-super-3-120b".into(),
        name: "NVIDIA Nemotron 3 Super 120B A12B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.65,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_68() -> [(&'static str, Model); 4] {
    [
        ("openai.gpt-oss-120b-1:0", openai_dot_gpt_oss_120b_1_0()),
        ("openai.gpt-oss-20b-1:0", openai_dot_gpt_oss_20b_1_0()),
        (
            "openai.gpt-oss-safeguard-120b",
            openai_dot_gpt_oss_safeguard_120b(),
        ),
        (
            "openai.gpt-oss-safeguard-20b",
            openai_dot_gpt_oss_safeguard_20b(),
        ),
    ]
}
/// Construct the recorded descriptor for this model.
fn openai_dot_gpt_oss_120b_1_0() -> Model {
    Model {
        id: "openai.gpt-oss-120b-1:0".into(),
        name: "gpt-oss-120b".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
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
fn openai_dot_gpt_oss_20b_1_0() -> Model {
    Model {
        id: "openai.gpt-oss-20b-1:0".into(),
        name: "gpt-oss-20b".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.3,
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
fn openai_dot_gpt_oss_safeguard_120b() -> Model {
    Model {
        id: "openai.gpt-oss-safeguard-120b".into(),
        name: "GPT OSS Safeguard 120B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
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
fn openai_dot_gpt_oss_safeguard_20b() -> Model {
    Model {
        id: "openai.gpt-oss-safeguard-20b".into(),
        name: "GPT OSS Safeguard 20B".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 128_000.0,
        max_tokens: 4096.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_72() -> [(&'static str, Model); 7] {
    [
        (
            "qwen.qwen3-235b-a22b-2507-v1:0",
            qwen_dot_qwen3_235b_a22b_2507_v1_0(),
        ),
        ("qwen.qwen3-32b-v1:0", qwen_dot_qwen3_32b_v1_0()),
        (
            "qwen.qwen3-coder-30b-a3b-v1:0",
            qwen_dot_qwen3_coder_30b_a3b_v1_0(),
        ),
        (
            "qwen.qwen3-coder-480b-a35b-v1:0",
            qwen_dot_qwen3_coder_480b_a35b_v1_0(),
        ),
        ("qwen.qwen3-coder-next", qwen_dot_qwen3_coder_next()),
        ("qwen.qwen3-next-80b-a3b", qwen_dot_qwen3_next_80b_a3b()),
        ("qwen.qwen3-vl-235b-a22b", qwen_dot_qwen3_vl_235b_a22b()),
    ]
}
/// Construct the recorded descriptor for this model.
fn qwen_dot_qwen3_235b_a22b_2507_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-235b-a22b-2507-v1:0".into(),
        name: "Qwen3 235B A22B 2507".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 0.88,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_dot_qwen3_32b_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-32b-v1:0".into(),
        name: "Qwen3 32B (dense)".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 16_384.0,
        max_tokens: 16_384.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_dot_qwen3_coder_30b_a3b_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-coder-30b-a3b-v1:0".into(),
        name: "Qwen3 Coder 30B A3B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.15,
            output: 0.6,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_144.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_dot_qwen3_coder_480b_a35b_v1_0() -> Model {
    Model {
        id: "qwen.qwen3-coder-480b-a35b-v1:0".into(),
        name: "Qwen3 Coder 480B A35B Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 1.8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_dot_qwen3_coder_next() -> Model {
    Model {
        id: "qwen.qwen3-coder-next".into(),
        name: "Qwen3 Coder Next".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.22,
            output: 1.8,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 131_072.0,
        max_tokens: 65_536.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_dot_qwen3_next_80b_a3b() -> Model {
    Model {
        id: "qwen.qwen3-next-80b-a3b".into(),
        name: "Qwen/Qwen3-Next-80B-A3B-Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.14,
            output: 1.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn qwen_dot_qwen3_vl_235b_a22b() -> Model {
    Model {
        id: "qwen.qwen3-vl-235b-a22b".into(),
        name: "Qwen/Qwen3-VL-235B-A22B-Instruct".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![ModelInput::Text, ModelInput::Image],
        cost: ModelCost {
            input: 0.3,
            output: 1.5,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 262_000.0,
        max_tokens: 262_000.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_88() -> [(&'static str, Model); 2] {
    [
        ("writer.palmyra-x4-v1:0", writer_dot_palmyra_x4_v1_0()),
        ("writer.palmyra-x5-v1:0", writer_dot_palmyra_x5_v1_0()),
    ]
}
/// Construct the recorded descriptor for this model.
fn writer_dot_palmyra_x4_v1_0() -> Model {
    Model {
        id: "writer.palmyra-x4-v1:0".into(),
        name: "Palmyra X4".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 2.5,
            output: 10.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 122_880.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn writer_dot_palmyra_x5_v1_0() -> Model {
    Model {
        id: "writer.palmyra-x5-v1:0".into(),
        name: "Palmyra X5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 6.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 1_040_000.0,
        max_tokens: 8192.0,
        headers: None,
        compat: None,
    }
}

/// Assemble a batch of descriptors in registry order.
pub(super) fn models_90() -> [(&'static str, Model); 3] {
    [
        ("zai.glm-4.7", zai_dot_glm_4_dot_7()),
        ("zai.glm-4.7-flash", zai_dot_glm_4_dot_7_flash()),
        ("zai.glm-5", zai_dot_glm_5()),
    ]
}
/// Construct the recorded descriptor for this model.
fn zai_dot_glm_4_dot_7() -> Model {
    Model {
        id: "zai.glm-4.7".into(),
        name: "GLM-4.7".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.6,
            output: 2.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 204_800.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn zai_dot_glm_4_dot_7_flash() -> Model {
    Model {
        id: "zai.glm-4.7-flash".into(),
        name: "GLM-4.7-Flash".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 0.07,
            output: 0.4,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 200_000.0,
        max_tokens: 131_072.0,
        headers: None,
        compat: None,
    }
}

/// Construct the recorded descriptor for this model.
fn zai_dot_glm_5() -> Model {
    Model {
        id: "zai.glm-5".into(),
        name: "GLM-5".into(),
        api: "bedrock-converse-stream".into(),
        provider: "amazon-bedrock".into(),
        base_url: "https://bedrock-runtime.us-east-1.amazonaws.com".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost {
            input: 1.0,
            output: 3.2,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 202_752.0,
        max_tokens: 101_376.0,
        headers: None,
        compat: None,
    }
}
