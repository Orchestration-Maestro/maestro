//! Tool argument roots through session history, its file and its replay.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

use maestro_models::{
    AssistantContent, Message, Model, ModelCost, ModelInput, ToolCall, transform_messages,
};
use maestro_session::SessionManager;
use maestro_session::session_manager::ReadonlySessionManager;
use maestro_test_directory::Scratch;
use serde_json::{Value, json};

/// Every root kind a stored call can carry.
fn roots() -> Vec<Value> {
    vec![
        json!("text"),
        json!(42),
        json!(false),
        Value::Null,
        json!([]),
        json!([1, 1]),
        json!({"q": 1}),
    ]
}

/// One stored assistant message carrying a single call with `arguments`.
fn assistant(arguments: &Value) -> Value {
    json!({
        "role": "assistant",
        "content": [{
            "type": "toolCall",
            "id": "call",
            "name": "echo",
            "arguments": arguments,
            "thoughtSignature": "signature",
        }],
        "api": "anthropic-messages",
        "provider": "provider",
        "model": "model",
        "stopReason": "toolUse",
        "timestamp": 3,
        "usage": {"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,
        "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},
    })
}

/// A target model for the projection step.
fn model() -> Model {
    Model {
        id: "target".into(),
        name: "Target".into(),
        api: "anthropic-messages".into(),
        provider: "target-provider".into(),
        base_url: "https://example.invalid".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec![ModelInput::Text],
        cost: ModelCost::default(),
        context_window: 1000.0,
        max_tokens: 100.0,
        headers: None,
        compat: None,
    }
}

/// The single call of one assistant message, when it has one.
fn projected_call(message: &Message) -> Option<&ToolCall> {
    let Message::Assistant(assistant) = message else {
        return None;
    };
    match assistant.content.first() {
        Some(AssistantContent::ToolCall(call)) => Some(call),
        _ => None,
    }
}

/// The calls a projected history carries, in order.
fn calls(messages: &[Message]) -> Vec<&ToolCall> {
    messages.iter().filter_map(projected_call).collect()
}

#[test]
fn session_history_replays_each_tool_argument_root() {
    let mut manager = SessionManager::in_memory(Some("/work")).expect("a memory session");
    for arguments in roots() {
        manager
            .append_message(assistant(&arguments))
            .expect("the call is appended");
    }
    let context = manager.build_session_context().expect("the history builds");
    let messages: Vec<Message> = context
        .messages
        .iter()
        .cloned()
        .map(|message| serde_json::from_value(message).expect("the message decodes"))
        .collect();
    let replayed = calls(&messages);
    assert_eq!(replayed.len(), roots().len());
    for (call, arguments) in replayed.iter().zip(roots()) {
        assert_eq!(call.arguments, arguments);
        assert_eq!(call.thought_signature.as_deref(), Some("signature"));
    }
    let projected = transform_messages(&messages, &model(), None);
    let projected = calls(&projected);
    assert_eq!(projected.len(), roots().len());
    for (call, arguments) in projected.iter().zip(roots()) {
        assert_eq!(call.arguments, arguments);
        assert_eq!(call.id, "call");
        assert_eq!(call.name, "echo");
    }
}

#[test]
fn session_jsonl_reopens_tool_argument_roots() {
    let scratch = Scratch::new().expect("a disposable root");
    let directory = scratch.path().to_str().expect("a UTF-8 path");
    let mut manager =
        SessionManager::create("/work", Some(directory), directory).expect("a new session");
    for arguments in roots() {
        manager
            .append_message(assistant(&arguments))
            .expect("the call is appended");
    }
    let file = std::path::Path::new(manager.get_session_dir())
        .join(manager.get_session_file().expect("a stored session"))
        .to_string_lossy()
        .into_owned();
    drop(manager);
    let stored = std::fs::read_to_string(&file).expect("the session file reads");
    for arguments in roots() {
        let line = stored
            .lines()
            .find(|line| {
                serde_json::from_str::<Value>(line).is_ok_and(|entry| {
                    let call = &entry["message"]["content"][0];
                    call["type"] == json!("toolCall") && call["arguments"] == arguments
                })
            })
            .unwrap_or_else(|| panic!("no stored line carries {arguments}"));
        let entry: Value = serde_json::from_str(line).expect("the line decodes");
        assert_eq!(entry["message"]["content"][0]["type"], json!("toolCall"));
        assert_eq!(entry["message"]["content"][0]["id"], json!("call"));
    }
    let reopened = SessionManager::open(&file, None, Some("/work")).expect("the session reopens");
    let context = reopened
        .build_session_context()
        .expect("the history builds");
    let messages: Vec<Message> = context
        .messages
        .iter()
        .cloned()
        .map(|message| serde_json::from_value(message).expect("the message decodes"))
        .collect();
    let replayed = calls(&messages);
    assert_eq!(replayed.len(), roots().len());
    for (call, arguments) in replayed.iter().zip(roots()) {
        assert_eq!(call.arguments, arguments);
    }
}
