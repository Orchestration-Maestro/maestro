use super::{FauxAssistantContent, FauxAssistantMessageOptions, FauxToolCallOptions};
use crate::{
    AssistantContent, AssistantMessage, JsonObject, StopReason, TextContent, ThinkingContent,
    ToolCall, Usage, UsageCost, records::diagnostics::timestamp_now,
};

/// Generate an independent clock-prefixed opaque identity.
pub(super) fn random_id(prefix: &str) -> String {
    format!(
        "{prefix}:{:.0}:{:x}",
        timestamp_now(),
        rand::random::<u64>()
    )
}
/// Construct fresh zero token counts and costs.
pub(super) fn zero_usage() -> Usage {
    Usage {
        input: 0.0,
        output: 0.0,
        cache_read: 0.0,
        cache_write: 0.0,
        total_tokens: 0.0,
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    }
}
/// Construct unsigned text content.
pub fn faux_text(text: impl Into<String>) -> TextContent {
    TextContent {
        text: text.into(),
        text_signature: None,
    }
}
/// Construct unsigned thinking content.
pub fn faux_thinking(thinking: impl Into<String>) -> ThinkingContent {
    ThinkingContent {
        thinking: thinking.into(),
        thinking_signature: None,
        redacted: None,
    }
}
/// Construct a tool call with supplied arguments and an optional identity.
pub fn faux_tool_call(
    name: impl Into<String>,
    arguments: JsonObject,
    options: FauxToolCallOptions,
) -> ToolCall {
    ToolCall {
        id: options.id.unwrap_or_else(|| random_id("tool")),
        name: name.into(),
        arguments,
        thought_signature: None,
    }
}
/// Construct an owned assistant message with zero usage.
pub fn faux_assistant_message(
    content: impl Into<FauxAssistantContent>,
    options: FauxAssistantMessageOptions,
) -> AssistantMessage {
    let content = match content.into() {
        FauxAssistantContent::Text(text) => vec![AssistantContent::Text(faux_text(text))],
        FauxAssistantContent::Block(block) => vec![block],
        FauxAssistantContent::Blocks(blocks) => blocks,
    };
    AssistantMessage {
        content,
        api: "faux".into(),
        provider: "faux".into(),
        model: "faux-1".into(),
        response_model: None,
        response_id: options.response_id,
        diagnostics: None,
        usage: zero_usage(),
        stop_reason: options.stop_reason.unwrap_or(StopReason::Stop),
        error_message: options.error_message,
        timestamp: options.timestamp.unwrap_or_else(timestamp_now),
    }
}

/// Build a model descriptor from optional overrides.
pub(super) fn model(
    definition: super::FauxModelDefinition,
    api: &str,
    provider: &str,
) -> crate::Model {
    crate::Model {
        name: definition.name.unwrap_or_else(|| definition.id.clone()),
        id: definition.id,
        api: api.into(),
        provider: provider.into(),
        base_url: "http://localhost:0".into(),
        reasoning: definition.reasoning.unwrap_or(false),
        thinking_level_map: None,
        input: definition
            .input
            .unwrap_or_else(|| vec![crate::ModelInput::Text, crate::ModelInput::Image]),
        cost: definition.cost.unwrap_or_default(),
        context_window: definition.context_window.unwrap_or(128_000.0),
        max_tokens: definition.max_tokens.unwrap_or(16_384.0),
        headers: None,
        compat: None,
    }
}
