// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 9] {
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
