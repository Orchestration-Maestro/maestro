//! Emission of compatibility fields populated by catalog acquisition.
use crate::ModelCompat;
use std::fmt::Write as _;
/// Emit existing typed variants without a runtime JSON decoder.
pub(super) fn compat(compat: Option<&ModelCompat>) -> String {
    match compat {
        None => "None".into(),
        Some(ModelCompat::OpenAICompletions(compat)) => completions(compat),
        Some(ModelCompat::OpenAIResponses(compat)) => format!(
            "Some(crate::ModelCompat::OpenAIResponses(crate::OpenAIResponsesCompat {{ send_session_id_header: {:?}, supports_long_cache_retention: {:?} }}))",
            compat.send_session_id_header, compat.supports_long_cache_retention
        ),
        Some(ModelCompat::AnthropicMessages(compat)) => format!(
            "Some(crate::ModelCompat::AnthropicMessages(crate::AnthropicMessagesCompat {{ supports_eager_tool_input_streaming: {:?}, supports_long_cache_retention: {:?} }}))",
            compat.supports_eager_tool_input_streaming, compat.supports_long_cache_retention
        ),
    }
}
/// Emit only catalog-populated completion fields with typed enum names.
fn completions(compat: &crate::OpenAICompletionsCompat) -> String {
    let mut fields = String::new();
    for (name, value) in [
        ("supports_store", compat.supports_store),
        ("supports_developer_role", compat.supports_developer_role),
        (
            "supports_reasoning_effort",
            compat.supports_reasoning_effort,
        ),
        (
            "requires_reasoning_content_on_assistant_messages",
            compat.requires_reasoning_content_on_assistant_messages,
        ),
        ("supports_strict_mode", compat.supports_strict_mode),
        ("zai_tool_stream", compat.zai_tool_stream),
        (
            "send_session_affinity_headers",
            compat.send_session_affinity_headers,
        ),
    ] {
        if let Some(value) = value {
            let _ = writeln!(fields, "{name}: Some({value}),");
        }
    }
    if let Some(value) = compat.max_tokens_field {
        let _ = writeln!(
            fields,
            "max_tokens_field: Some(crate::MaxTokensField::{value:?}),"
        );
    }
    if let Some(value) = compat.thinking_format {
        let _ = writeln!(
            fields,
            "thinking_format: Some(crate::ThinkingFormat::{value:?}),"
        );
    }
    if let Some(value) = compat.cache_control_format {
        let _ = writeln!(
            fields,
            "cache_control_format: Some(crate::CacheControlFormat::{value:?}),"
        );
    }
    format!(
        "Some(crate::ModelCompat::OpenAICompletions(Box::new(crate::OpenAICompletionsCompat {{\n{fields}..crate::OpenAICompletionsCompat::default()\n}})))"
    )
}
