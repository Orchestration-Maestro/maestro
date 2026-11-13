//! Message, usage, template and tool records as the recorded cases spell them.
use maestro_extensions::ExtensionContext;
use maestro_models::{
    AssistantMessage, FauxAssistantMessageOptions, Local, StopReason, Tool, faux_assistant_message,
};
use maestro_resources::{PromptTemplate, SyntheticSourceOptions, create_synthetic_source_info};
use maestro_tools::ToolDefinitionRecord;
use serde_json::{Value, json};
use std::rc::Rc;

/// Base time of the recorded messages.
pub const TS: u64 = 1_700_000_000_000;

/// Recorded usage with an optional total.
#[must_use]
pub fn usage(tokens: [u64; 4], cost: f64, total_tokens: Option<u64>) -> Value {
    let [input, output, cache_read, cache_write] = tokens;
    let usage = json!({
        "input": input, "output": output, "cacheRead": cache_read, "cacheWrite": cache_write,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": cost},
    });
    match total_tokens {
        Some(total) => super::with_field(usage, "totalTokens", json!(total)),
        None => usage,
    }
}

/// An answer record with `text`, then the fields of the object `extra` replacing the
/// defaults.
#[must_use]
pub fn assistant(text: &str, offset: u64, extra: &Value) -> Value {
    let mut message = json!({
        "role": "assistant", "content": [{"type": "text", "text": text}], "api": "faux",
        "provider": "faux", "model": "plain", "usage": usage([1, 1, 0, 0], 0.0, Some(2)),
        "stopReason": "stop", "timestamp": TS + offset,
    });
    for (key, value) in extra.as_object().into_iter().flatten() {
        message = super::with_field(message, key, value.clone());
    }
    message
}

/// A scripted answer with an optional stop reason and error text.
#[must_use]
pub fn answer(text: &str, stop: Option<StopReason>, error: Option<&str>) -> AssistantMessage {
    faux_assistant_message(
        text,
        FauxAssistantMessageOptions {
            stop_reason: stop,
            error_message: error.map(str::to_owned),
            ..FauxAssistantMessageOptions::default()
        },
    )
}

/// The text of a message record: plain content, joined text blocks, or its summary.
#[must_use]
pub fn message_text(record: &Value) -> Value {
    match &record["content"] {
        Value::String(text) => json!(text),
        Value::Array(blocks) => json!(
            blocks
                .iter()
                .filter(|block| block["type"] == "text")
                .filter_map(|block| block["text"].as_str())
                .collect::<String>()
        ),
        _ => record.get("summary").cloned().unwrap_or(Value::Null),
    }
}

/// A prompt template named `name` with `content`.
#[must_use]
pub fn template(name: &str, content: &str) -> PromptTemplate {
    PromptTemplate {
        name: name.into(),
        description: String::new(),
        argument_hint: None,
        content: content.into(),
        source_info: create_synthetic_source_info(
            "plan.md".into(),
            SyntheticSourceOptions {
                source: "local".into(),
                scope: None,
                origin: None,
                base_dir: None,
            },
        ),
        file_path: "plan.md".into(),
    }
}

/// A native tool definition.
pub type Definition = ToolDefinitionRecord<ExtensionContext, Value, Value, (), Local, Local>;

/// A tool definition with prompt text whose execution returns nothing.
#[must_use]
pub fn tool_definition(
    name: &str,
    snippet: Option<&str>,
    guidelines: Option<&[&str]>,
) -> Definition {
    ToolDefinitionRecord {
        definition: Tool {
            name: name.into(),
            description: format!("{name} tool"),
            parameters: json!({"type": "object", "properties": {}}).into(),
        },
        label: name.into(),
        prompt_snippet: snippet.map(str::to_owned),
        prompt_guidelines: guidelines
            .map(|lines| lines.iter().map(|line| (*line).to_owned()).collect()),
        render_shell: None,
        prepare_arguments: None,
        execution_mode: None,
        execute: Rc::new(|_, _, _, _, _| {
            Box::pin(async {
                Ok(maestro_agent::AgentToolResult {
                    content: vec![],
                    details: None,
                    terminate: None,
                })
            })
        }),
        render_call: None,
        render_result: None,
    }
}
