//! Conversation conversion and HTTP parameters for the custom message protocol.
use maestro_extensions_wasm::{
    AssistantContent, Context, ExtensionResult, Message, Model, RequestedThinkingLevel,
    StreamOptionsInput, ThinkingBudgets, ToolResultMessage, UserBlock, UserContent, http::Request,
};
use serde_json::{Value, json};

/// Subscription identity required by the remote service.
const SUBSCRIPTION_SYSTEM_IDENTITY: &str =
    "You are Claude Code, Anthropic's official CLI for Claude.";
/// Subscription client identity required by the remote service.
const SUBSCRIPTION_USER_AGENT: &str = "claude-cli/2.1.2 (external, cli)";
/// Streaming features used for both authentication modes.
const STREAM_BETAS: &str = "fine-grained-tool-streaming-2025-05-14,interleaved-thinking-2025-05-14";
/// Canonical tool names accepted by the subscription endpoint.
const SUBSCRIPTION_TOOLS: [&str; 10] = [
    "Read",
    "Write",
    "Edit",
    "Bash",
    "Grep",
    "Glob",
    "AskUserQuestion",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
];

/// Whether the key selects subscription authentication.
pub fn subscription(key: &str) -> bool {
    key.contains("sk-ant-oat")
}

/// Maps an authored tool name to the subscription endpoint's spelling.
pub fn wire_tool_name(name: &str) -> String {
    let lower = name.to_lowercase();
    SUBSCRIPTION_TOOLS
        .iter()
        .find(|known| known.to_lowercase() == lower)
        .map_or_else(|| name.to_owned(), |known| (*known).to_owned())
}

/// Builds one HTTP attempt without introducing a second retry owner.
///
/// # Errors
/// Returns missing authentication, schema serialization or malformed reasoning errors.
pub fn build(
    model: &Model,
    context: &Context,
    options: Option<&StreamOptionsInput>,
) -> ExtensionResult<Request> {
    let common = options.map(StreamOptionsInput::common);
    let key = common
        .and_then(|common| common.api_key.as_deref())
        .filter(|key| !key.is_empty())
        .ok_or("Missing custom provider authentication")?;
    let subscribed = subscription(key);
    let mut body = json!({
        "model":model.id, "messages":messages(&context.messages, subscribed),
        "max_tokens":common.and_then(|common| common.max_tokens)
            .filter(|tokens| *tokens != 0.0 && !tokens.is_nan())
            .unwrap_or_else(|| (model.max_tokens / 3.0).trunc()),
        "stream":true
    });
    let system = system(context.system_prompt.as_deref(), subscribed);
    if !system.is_empty() {
        body["system"] = Value::Array(system);
    }
    if let Some(tools) = &context.tools {
        body["tools"] = Value::Array(tools.iter().map(|tool| {
            Ok(json!({
                "name":if subscribed { wire_tool_name(&tool.name) } else { tool.name.clone() },
                "description":tool.description,
                "input_schema":serde_json::to_value(&tool.parameters).map_err(|error| error.to_string())?
            }))
        }).collect::<ExtensionResult<_>>()?);
    }
    if model.reasoning
        && let Some(thinking) = thinking(options)?
    {
        body["thinking"] = thinking;
    }
    Ok(Request {
        url: format!("{}/v1/messages", model.base_url.trim_end_matches('/')),
        method: Some("POST".into()),
        headers: headers(key, subscribed),
        body: Some(serde_json::to_vec(&body).map_err(|error| error.to_string())?),
        signal: common.and_then(|common| common.signal.clone()),
    })
}

/// The required protocol headers, with exactly one credential representation.
fn headers(key: &str, subscribed: bool) -> Vec<(String, String)> {
    let mut headers = vec![
        ("content-type".into(), "application/json".into()),
        ("accept".into(), "application/json".into()),
        ("anthropic-version".into(), "2023-06-01".into()),
        (
            "anthropic-dangerous-direct-browser-access".into(),
            "true".into(),
        ),
    ];
    if subscribed {
        headers.extend([
            ("authorization".into(), format!("Bearer {key}")),
            (
                "anthropic-beta".into(),
                format!("claude-code-20250219,oauth-2025-04-20,{STREAM_BETAS}"),
            ),
            ("user-agent".into(), SUBSCRIPTION_USER_AGENT.into()),
            ("x-app".into(), "cli".into()),
        ]);
    } else {
        headers.extend([
            ("x-api-key".into(), key.into()),
            ("anthropic-beta".into(), STREAM_BETAS.into()),
        ]);
    }
    headers
}

/// Cacheable system blocks preserve the supplied instruction text.
fn system(prompt: Option<&str>, subscribed: bool) -> Vec<Value> {
    let mut blocks = Vec::new();
    if subscribed {
        blocks.push(json!({"type":"text","text":SUBSCRIPTION_SYSTEM_IDENTITY,"cache_control":{"type":"ephemeral"}}));
    }
    if let Some(prompt) = prompt.filter(|prompt| !prompt.is_empty()) {
        blocks.push(json!({"type":"text","text":prompt,"cache_control":{"type":"ephemeral"}}));
    }
    blocks
}

/// Raw and simple invocations share the same reasoning field types.
fn thinking(options: Option<&StreamOptionsInput>) -> ExtensionResult<Option<Value>> {
    match options {
        Some(StreamOptionsInput::Simple(options)) => Ok(thinking_budget(
            options.reasoning,
            options.thinking_budgets.as_ref(),
        )),
        Some(StreamOptionsInput::Raw(options)) => {
            let level = serde_json::from_value::<Option<RequestedThinkingLevel>>(
                options
                    .extra
                    .get("reasoning")
                    .cloned()
                    .unwrap_or(Value::Null),
            )
            .map_err(|error| format!("Invalid reasoning: {error}"))?;
            let budgets = serde_json::from_value::<Option<ThinkingBudgets>>(
                options
                    .extra
                    .get("thinkingBudgets")
                    .cloned()
                    .unwrap_or(Value::Null),
            )
            .map_err(|error| format!("Invalid thinkingBudgets: {error}"))?;
            Ok(thinking_budget(level, budgets.as_ref()))
        }
        None => Ok(None),
    }
}

/// Reasoning uses the authored effort budgets when the caller supplies none.
fn thinking_budget(
    level: Option<RequestedThinkingLevel>,
    budgets: Option<&ThinkingBudgets>,
) -> Option<Value> {
    let budget = match level? {
        RequestedThinkingLevel::Minimal => budgets.and_then(|b| b.minimal).unwrap_or(1024.0),
        RequestedThinkingLevel::Low => budgets.and_then(|b| b.low).unwrap_or(4096.0),
        RequestedThinkingLevel::Medium => budgets.and_then(|b| b.medium).unwrap_or(10_240.0),
        RequestedThinkingLevel::High => budgets.and_then(|b| b.high).unwrap_or(20_480.0),
        RequestedThinkingLevel::Xhigh => 10_240.0,
    };
    Some(json!({"type":"enabled","budget_tokens":budget}))
}

/// Groups adjacent tool results and caches the final user block when it is structured.
fn messages(messages: &[Message], subscribed: bool) -> Vec<Value> {
    let mut converted = Vec::new();
    let mut messages = messages.iter().peekable();
    while let Some(message) = messages.next() {
        match message {
            Message::User(user) => {
                if let Some(content) = user_content(&user.content) {
                    converted.push(json!({"role":"user","content":content}));
                }
            }
            Message::Assistant(assistant) => {
                let blocks: Vec<_> = assistant
                    .content
                    .iter()
                    .filter_map(|block| assistant_block(block, subscribed))
                    .collect();
                if !blocks.is_empty() {
                    converted.push(json!({"role":"assistant","content":blocks}));
                }
            }
            Message::ToolResult(result) => {
                let mut results = vec![tool_result(result)];
                while let Some(Message::ToolResult(result)) =
                    messages.next_if(|message| matches!(message, Message::ToolResult(_)))
                {
                    results.push(tool_result(result));
                }
                converted.push(json!({"role":"user","content":results}));
            }
        }
    }
    if let Some(last) = converted.last_mut().filter(|last| last["role"] == "user")
        && let Some(block) = last["content"]
            .as_array_mut()
            .and_then(|blocks| blocks.last_mut())
    {
        block["cache_control"] = json!({"type":"ephemeral"});
    }
    converted
}

/// Converts text and image records without changing valid Unicode text.
fn user_block(block: &UserBlock) -> Value {
    match block {
        UserBlock::Text(text) => json!({"type":"text","text":text.text}),
        UserBlock::Image(image) => {
            json!({"type":"image","source":{"type":"base64","media_type":image.mime_type,"data":image.data}})
        }
    }
}

/// Keeps signed thinking opaque and sends unsigned thinking as ordinary text.
fn assistant_block(block: &AssistantContent, subscribed: bool) -> Option<Value> {
    match block {
        AssistantContent::Text(text) if !text.text.trim().is_empty() => {
            Some(json!({"type":"text","text":text.text}))
        }
        AssistantContent::Thinking(thinking) if !thinking.thinking.trim().is_empty() => Some(
            if let Some(signature) = thinking
                .thinking_signature
                .as_ref()
                .filter(|signature| !signature.is_empty())
            {
                json!({"type":"thinking","thinking":thinking.thinking,"signature":signature})
            } else {
                json!({"type":"text","text":thinking.thinking})
            },
        ),
        AssistantContent::ToolCall(tool) => Some(json!({
            "type":"tool_use","id":tool.id,"name":if subscribed { wire_tool_name(&tool.name) } else { tool.name.clone() },"input":tool.arguments
        })),
        _ => None,
    }
}

/// A tool result is text when possible, or image blocks with an explanatory label.
fn tool_result(result: &ToolResultMessage) -> Value {
    let content = if result
        .content
        .iter()
        .any(|block| matches!(block, UserBlock::Image(_)))
    {
        let mut blocks: Vec<_> = result.content.iter().map(user_block).collect();
        if !result
            .content
            .iter()
            .any(|block| matches!(block, UserBlock::Text(_)))
        {
            blocks.insert(0, json!({"type":"text","text":"(see attached image)"}));
        }
        Value::Array(blocks)
    } else {
        json!(UserBlock::text_output(&result.content))
    };
    json!({"type":"tool_result","tool_use_id":result.tool_call_id,"content":content,"is_error":result.is_error})
}

/// Omits empty user records while retaining structured image/text input.
fn user_content(content: &UserContent) -> Option<Value> {
    match content {
        UserContent::Text(text) if !text.trim().is_empty() => Some(json!(text)),
        UserContent::Blocks(blocks) if !blocks.is_empty() => {
            Some(json!(blocks.iter().map(user_block).collect::<Vec<_>>()))
        }
        _ => None,
    }
}
