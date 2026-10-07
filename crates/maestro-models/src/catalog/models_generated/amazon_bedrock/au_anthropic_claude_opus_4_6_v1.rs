// Generated model descriptor data.
use crate::{Model, ModelCost, ModelInput, ModelThinkingLevel};
pub(super) fn models() -> [(&'static str, Model); 2] {
    [
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
