//! Conversion of conversation history and tool declarations into response wire items.

use std::collections::HashSet;
use std::hash::BuildHasher;
use std::iter::repeat_n;

use serde_json::{Map, Value, json};

use super::{ConvertResponsesMessagesOptions, ConvertResponsesToolsOptions, native};
use crate::providers::json_text::{compact_object, json_value, member, raw_json, raw_number};
use crate::{
    AssistantContent, AssistantMessage, Context, DiagnosticErrorInfo, Message, Model, ModelInput,
    TextContent, TextPhase, Tool, ToolCall, ToolResultMessage, UserBlock, UserContent, UserMessage,
    short_hash, transform_messages,
};

/// Output of a tool result that has no text.
const IMAGE_PLACEHOLDER: &str = "(see attached image)";
/// Longest identity part and message identifier, in UTF-16 units.
const ID_LIMIT: usize = 64;

/// Project the history for the model and convert it into response input items.
///
/// Tool-call identifiers of other models' turns are normalized for the model's provider when
/// that provider is in `allowed_tool_call_providers`; a provider outside it limits the whole
/// identifier to the safe characters.
///
/// # Errors
/// Fails when a reasoning signature is not JSON, or nests more than 127 containers, or when a
/// tool call's arguments cannot be written.
pub fn convert_responses_messages<S: BuildHasher>(
    model: &Model,
    context: &Context,
    allowed_tool_call_providers: &HashSet<String, S>,
    options: Option<&ConvertResponsesMessagesOptions>,
) -> Result<Vec<Value>, DiagnosticErrorInfo> {
    let listed = allowed_tool_call_providers.contains(&model.provider);
    let normalize = |id: &str, _: &Model, source: &AssistantMessage| {
        normalize_tool_call_id(id, model, source, listed)
    };
    let history = transform_messages(&context.messages, model, Some(&normalize));
    let mut items = Vec::with_capacity(history.len() + 1);
    let include_prompt = options.is_none_or(|options| options.include_system_prompt);
    if let Some(prompt) = context
        .system_prompt
        .as_deref()
        .filter(|prompt| include_prompt && !prompt.is_empty())
    {
        let role = if model.reasoning {
            "developer"
        } else {
            "system"
        };
        items.push(json!({"role": role, "content": prompt}));
    }
    let mut retained = 0;
    for message in &history {
        let converted = match message {
            Message::User(user) => convert_user(user),
            Message::Assistant(assistant) => convert_assistant(assistant, model, retained)?,
            Message::ToolResult(result) => vec![convert_tool_result(result, model)],
        };
        if !converted.is_empty() {
            items.extend(converted);
            retained += 1;
        }
    }
    Ok(items)
}

/// Convert tool declarations; `strict` defaults to `false` and `None` sends `null`.
#[must_use]
pub fn convert_responses_tools(
    tools: &[Tool],
    options: Option<&ConvertResponsesToolsOptions>,
) -> Vec<Value> {
    let strict = options
        .map_or(Some(false), |options| options.strict)
        .map_or(Value::Null, Value::Bool);
    tools
        .iter()
        .map(|tool| {
            json!({"type": "function", "name": tool.name, "description": tool.description,
                "parameters": tool.parameters, "strict": strict})
        })
        .collect()
}

/// Limit an identifier part to `[A-Za-z0-9_-]`, one `_` per UTF-16 unit replaced, at most 64
/// units, without trailing underscores.
fn normalize_id_part(part: &str) -> String {
    let limited: String = part
        .chars()
        .flat_map(|character| {
            let kept = character.is_ascii_alphanumeric() || matches!(character, '_' | '-');
            let units = if kept { 1 } else { character.len_utf16() };
            repeat_n(if kept { character } else { '_' }, units)
        })
        .take(ID_LIMIT)
        .collect();
    limited.trim_end_matches('_').to_owned()
}

/// Normalize the identifier of a tool call made by another model.
///
/// A model whose provider is not `listed`, or an identifier without `|`, gets one part.
/// Otherwise the first two `|` parts are kept: the call part normalized, the item part hashed
/// when another provider or protocol made the call and normalized otherwise, and prefixed with
/// `fc_` when it does not start with it.
fn normalize_tool_call_id(
    id: &str,
    model: &Model,
    source: &AssistantMessage,
    listed: bool,
) -> String {
    let Some((call_id, rest)) = id.split_once('|').filter(|_| listed) else {
        return normalize_id_part(id);
    };
    let item = rest.split('|').next().unwrap_or_default();
    let foreign = source.provider != model.provider || source.api != model.api;
    let item = if foreign {
        format!("fc_{}", short_hash(item))
    } else {
        normalize_id_part(item)
    };
    let item = if item.starts_with("fc_") {
        item
    } else {
        normalize_id_part(&format!("fc_{item}"))
    };
    format!("{}|{item}", normalize_id_part(call_id))
}

/// A text input part.
fn input_text(text: &str) -> Value {
    json!({"type": "input_text", "text": text})
}

/// An image input part carrying the image as a data URL.
fn input_image(mime_type: &str, data: &str) -> Value {
    json!({"type": "input_image", "detail": "auto",
        "image_url": format!("data:{mime_type};base64,{data}")})
}

/// Convert a user turn; an empty block list produces no item.
fn convert_user(user: &UserMessage) -> Vec<Value> {
    let parts: Vec<Value> = match &user.content {
        UserContent::Text(text) => vec![input_text(text)],
        UserContent::Blocks(blocks) => blocks
            .iter()
            .map(|block| match block {
                UserBlock::Text(text) => input_text(&text.text),
                UserBlock::Image(image) => input_image(&image.mime_type, &image.data),
            })
            .collect(),
    };
    if parts.is_empty() {
        return Vec::new();
    }
    vec![json!({"role": "user", "content": parts})]
}

/// Convert an assistant turn into reasoning items, messages and function calls in order.
fn convert_assistant(
    assistant: &AssistantMessage,
    model: &Model,
    retained: usize,
) -> Result<Vec<Value>, DiagnosticErrorInfo> {
    let other_model = assistant.model != model.id
        && assistant.provider == model.provider
        && assistant.api == model.api;
    let mut items = Vec::new();
    for block in &assistant.content {
        match block {
            AssistantContent::Thinking(thinking) => {
                let signature = thinking.thinking_signature.as_deref();
                if let Some(signature) = signature.filter(|signature| !signature.is_empty()) {
                    let raw = raw_json(signature).map_err(|error| native(&error))?;
                    items.push(json_value(raw).map_err(|error| native(&error))?);
                }
            }
            AssistantContent::Text(text) => items.push(output_message(text, retained)),
            AssistantContent::ToolCall(call) => items.push(function_call(call, other_model)?),
        }
    }
    Ok(items)
}

/// The identity and phase a text signature carries.
struct TextIdentity {
    /// Message identifier.
    id: String,
    /// Message phase, when the signature names a known one.
    phase: Option<TextPhase>,
}

/// Read a text signature: version-one JSON with a string `id`, otherwise the whole signature
/// is the identifier. An empty signature has no identity.
fn text_identity(signature: Option<&str>) -> Option<TextIdentity> {
    let signature = signature.filter(|signature| !signature.is_empty())?;
    let structured = signature
        .starts_with('{')
        .then(|| raw_json(signature).ok())
        .flatten()
        .filter(|raw| member(raw, "v").and_then(raw_number) == Some(1.0))
        .and_then(|raw| {
            let text = |name| member(raw, name).and_then(|v| serde_json::from_str(v.get()).ok());
            Some(TextIdentity {
                id: text("id")?,
                phase: member(raw, "phase").and_then(|v| serde_json::from_str(v.get()).ok()),
            })
        });
    Some(structured.unwrap_or_else(|| TextIdentity {
        id: signature.to_owned(),
        phase: None,
    }))
}

/// Convert an assistant text block into a completed output message. An identifier of more than
/// 64 UTF-16 units is replaced by a hash of it, and a missing one by the message position.
fn output_message(text: &TextContent, retained: usize) -> Value {
    let identity = text_identity(text.text_signature.as_deref());
    let id = match identity.as_ref().map(|identity| identity.id.as_str()) {
        Some(id) if id.encode_utf16().count() > ID_LIMIT => format!("msg_{}", short_hash(id)),
        Some(id) if !id.is_empty() => id.to_owned(),
        _ => format!("msg_{retained}"),
    };
    let mut message = json!({"type": "message", "role": "assistant",
        "content": [{"type": "output_text", "text": text.text, "annotations": []}],
        "status": "completed", "id": id});
    match identity.and_then(|identity| identity.phase) {
        Some(TextPhase::Commentary) => message["phase"] = json!("commentary"),
        Some(TextPhase::FinalAnswer) => message["phase"] = json!("final_answer"),
        None => {}
    }
    message
}

/// Convert a tool call. The item identifier follows the first `|`, and is dropped when another
/// model of the same provider made a call whose identifier starts with `fc_`, so the service
/// does not look for the reasoning item it was paired with.
fn function_call(call: &ToolCall, other_model: bool) -> Result<Value, DiagnosticErrorInfo> {
    let mut parts = call.id.split('|');
    let call_id = parts.next().unwrap_or_default();
    let item_id = parts
        .next()
        .filter(|item| !(other_model && item.starts_with("fc_")));
    let mut item = Map::new();
    item.insert("type".to_owned(), json!("function_call"));
    if let Some(item_id) = item_id {
        item.insert("id".to_owned(), json!(item_id));
    }
    item.insert("call_id".to_owned(), json!(call_id));
    item.insert("name".to_owned(), json!(call.name));
    let arguments = compact_object(&call.arguments).map_err(|error| native(&error))?;
    item.insert("arguments".to_owned(), json!(arguments));
    Ok(Value::Object(item))
}

/// Convert a tool result. Its text lines are joined; images follow the text when the model
/// reads images, and otherwise only the text, or a placeholder, is sent.
fn convert_tool_result(result: &ToolResultMessage, model: &Model) -> Value {
    let text = result
        .content
        .iter()
        .filter_map(|block| match block {
            UserBlock::Text(text) => Some(text.text.as_str()),
            UserBlock::Image(_) => None,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let images: Vec<Value> = result
        .content
        .iter()
        .filter_map(|block| match block {
            UserBlock::Image(image) => Some(input_image(&image.mime_type, &image.data)),
            UserBlock::Text(_) => None,
        })
        .collect();
    let output = if !images.is_empty() && model.input.contains(&ModelInput::Image) {
        let parts = (!text.is_empty()).then(|| input_text(&text));
        Value::Array(parts.into_iter().chain(images).collect())
    } else if text.is_empty() {
        json!(IMAGE_PLACEHOLDER)
    } else {
        json!(text)
    };
    let call_id = result.tool_call_id.split('|').next().unwrap_or_default();
    json!({"type": "function_call_output", "call_id": call_id, "output": output})
}
