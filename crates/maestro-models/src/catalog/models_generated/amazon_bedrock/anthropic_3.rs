// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput};
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
