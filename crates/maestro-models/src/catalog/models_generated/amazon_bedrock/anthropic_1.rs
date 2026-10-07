// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
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
