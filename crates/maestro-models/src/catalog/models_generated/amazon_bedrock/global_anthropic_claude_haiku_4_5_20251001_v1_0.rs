// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 7] {
    [
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
