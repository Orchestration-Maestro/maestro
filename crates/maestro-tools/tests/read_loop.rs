//! The wrapped read tool and the same caller over native and supplied operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]

mod read_support;

use maestro_agent::{
    AgentContext, AgentLoopConfig, AgentLoopOptions, AgentMessage, AgentTool, AgentToolResult,
    run_agent_loop,
};
use maestro_models::{
    AssistantContent, AssistantMessage, AssistantMessageEventStream, Context, DiagnosticCode,
    JsonObject, Message, ProviderObjects, SharedAssistantMessage, SimpleStreamOptions, StopReason,
    TextContent, ToolCall, UserContent, UserMessage,
};
use maestro_tools::{ReadOperations, ReadToolOptions, create_read_tool};
use read_support::*;
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, RwLock},
};

/// Run the erased read tool through its executable form.
fn read_through(
    options: ReadToolOptions,
    cwd: &str,
    path: &str,
) -> Result<AgentToolResult, maestro_models::DiagnosticErrorInfo> {
    let tool = create_read_tool(cwd.into(), options);
    run((tool.execute)(
        "id".into(),
        json!({"path": path}),
        None,
        None,
    ))
}

#[test]
fn native_and_supplied_reads_share_the_caller() {
    let dir = TempDir::new();
    dir.file("rel.txt", b"native bytes");
    let native = read_through(ReadToolOptions::default(), &dir.text(), "rel.txt").unwrap();
    assert_eq!(
        serde_json::to_value(&native.content).unwrap(),
        json!([text_json("native bytes")])
    );
    assert!(native.details.is_none());

    let ops = Arc::new(ScriptedOps::new(b"supplied bytes"));
    let options = || ReadToolOptions {
        auto_resize_images: None,
        operations: Some(Arc::clone(&ops) as Arc<dyn ReadOperations>),
    };
    let supplied = read_through(options(), "/work", "rel.txt").unwrap();
    assert_eq!(
        serde_json::to_value(&supplied.content).unwrap(),
        json!([text_json("supplied bytes")])
    );
    assert_eq!(
        ops.log(),
        [
            "access:/work/rel.txt",
            "mime:/work/rel.txt",
            "read:/work/rel.txt"
        ]
    );

    let failing = Arc::new(ScriptedOps::new(b"x").at(Stage::Access, Behavior::Fail, None));
    let options = ReadToolOptions {
        auto_resize_images: None,
        operations: Some(failing as Arc<dyn ReadOperations>),
    };
    assert_eq!(
        read_through(options, "/work", "rel.txt").err(),
        Some(ScriptedOps::failure(Stage::Access))
    );
    let missing = read_through(ReadToolOptions::default(), &dir.text(), "absent.txt")
        .err()
        .unwrap();
    assert_eq!(missing.code, Some(DiagnosticCode::Number(2.0)));
}

/// A shared user entry.
fn user(text: &str) -> AgentMessage {
    AgentMessage::User(Arc::new(RwLock::new(UserMessage {
        content: UserContent::Text(text.into()),
        timestamp: 1.0,
    })))
}

/// A completed assistant response, optionally requesting `calls` as `(id, tool, arguments)`.
fn assistant(text: &str, calls: &[(&str, &str, Value)]) -> SharedAssistantMessage {
    let mut content = vec![AssistantContent::Text(TextContent {
        text: text.into(),
        text_signature: None,
    })];
    content.extend(calls.iter().map(|(id, name, arguments)| {
        AssistantContent::ToolCall(ToolCall {
            id: (*id).into(),
            name: (*name).into(),
            arguments: arguments
                .as_object()
                .cloned()
                .unwrap_or_else(JsonObject::new),
            thought_signature: None,
        })
    }));
    Arc::new(RwLock::new(AssistantMessage {
        content,
        api: "fixture".into(),
        provider: "fixture".into(),
        model: "fixture".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: serde_json::from_value(json!({"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}})).unwrap(),
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 2.0,
    }))
}

/// A loop configuration that converts history without a provider.
fn config() -> AgentLoopConfig {
    AgentLoopConfig {
        thinking_budgets: None,
        model: Arc::new(RwLock::new(model(vec![]).as_ref().clone())),
        options: SimpleStreamOptions::default(),
        extra: JsonObject::new(),
        objects: ProviderObjects::default(),
        convert_to_llm: Arc::new(|history| {
            let messages = history
                .read()
                .unwrap()
                .iter()
                .map(|message| match message {
                    AgentMessage::User(value) => Message::User(value.read().unwrap().clone()),
                    AgentMessage::Assistant(value) => {
                        Message::Assistant(value.read().unwrap().clone())
                    }
                    AgentMessage::ToolResult(value) => {
                        Message::ToolResult(value.read().unwrap().clone())
                    }
                    AgentMessage::Custom(_) => unreachable!(),
                })
                .collect();
            Box::pin(async { Ok(messages) })
        }),
        transform_context: None,
        get_api_key: None,
        before_tool_call: None,
        after_tool_call: None,
        should_stop_after_turn: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        tool_execution: Some(maestro_agent::ToolExecutionMode::Sequential),
    }
}

/// A controlled model that answers each turn with the next prepared response.
fn responses(messages: Vec<SharedAssistantMessage>) -> AgentLoopOptions {
    let messages = Arc::new(Mutex::new(VecDeque::from(messages)));
    AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |_, _: Context, _, _| {
            let message = messages
                .lock()
                .unwrap()
                .pop_front()
                .expect("unexpected turn");
            Box::pin(async {
                let stream = AssistantMessageEventStream::new();
                stream.end(Some(message));
                Ok(stream)
            })
        })),
    }
}

/// A tool whose result carries an explicit null in `details`.
fn null_details_tool() -> AgentTool {
    AgentTool {
        definition: maestro_models::Tool {
            name: "nullish".into(),
            description: "explicit null".into(),
            parameters: json!({"type": "object"}),
        },
        label: "nullish".into(),
        prepare_arguments: None,
        execute: Arc::new(|_, _, _, _| {
            Box::pin(async {
                Ok(AgentToolResult {
                    content: Vec::new(),
                    details: Some(Value::Null),
                    terminate: None,
                })
            })
        }),
        execution_mode: None,
    }
}

/// Run one turn of tool calls through the raw loop and return the tool-result messages as the model receives them.
fn tool_results(dir: &TempDir, calls: &[(&str, &str, Value)]) -> Vec<Value> {
    let read = create_read_tool(dir.text(), ReadToolOptions::default());
    let context = AgentContext {
        system_prompt: "instructions".into(),
        messages: Arc::new(RwLock::new(Vec::new())),
        tools: Some(Arc::new(RwLock::new(vec![
            Arc::new(RwLock::new(read)),
            Arc::new(RwLock::new(null_details_tool())),
        ]))),
    };
    let emit =
        Arc::new(|_| -> maestro_models::BoxFuture<_> { Box::pin(std::future::ready(Ok(()))) });
    let returned = run(run_agent_loop(
        vec![user("go")],
        context,
        &config(),
        emit,
        responses(vec![assistant("call", calls), assistant("done", &[])]),
    ))
    .unwrap();
    returned
        .iter()
        .filter_map(|message| match message {
            AgentMessage::ToolResult(message) => {
                Some(serde_json::to_value(&*message.read().unwrap()).unwrap())
            }
            _ => None,
        })
        .collect()
}

#[test]
fn wrapped_read_runs_through_raw_agent_loop() {
    let dir = TempDir::new();
    dir.file("small.txt", b"Hello, world!\nLine 2");
    dir.file("long.txt", vec!["ab"; 2500].join("\n").as_bytes());
    dir.file("pixel", &base64_png());
    let wire = tool_results(
        &dir,
        &[
            ("small", "read", json!({"path": "small.txt"})),
            ("image", "read", json!({"path": "pixel"})),
            ("long", "read", json!({"path": "long.txt"})),
            ("missing", "read", json!({"path": "missing.txt"})),
            ("null", "nullish", json!({})),
        ],
    );
    assert_eq!(wire.len(), 5);
    assert_eq!(
        wire[0]["content"],
        json!([text_json("Hello, world!\nLine 2")])
    );
    assert!(wire[0].get("details").is_none() && wire[0]["isError"] == json!(false));
    assert_eq!(
        wire[1]["content"][0],
        text_json("Read image file [image/png]")
    );
    assert_eq!(wire[1]["content"][1]["mimeType"], "image/png");
    assert!(wire[1].get("details").is_none());
    let kept = vec!["ab"; 2000].join("\n");
    assert_eq!(
        wire[2]["content"],
        json!([text_json(&format!(
            "{kept}\n\n[Showing lines 1-2000 of 2500. Use offset=2001 to continue.]"
        ))])
    );
    assert_eq!(wire[2]["details"]["truncation"]["truncatedBy"], "lines");
    assert_eq!(wire[2]["details"]["truncation"]["totalLines"], 2500);
    assert_eq!(wire[3]["isError"], json!(true));
    assert_eq!(
        wire[3]["content"],
        json!([text_json("No such file or directory (os error 2)")])
    );
    assert!(wire[4].as_object().unwrap().contains_key("details"));
    assert_eq!(wire[4]["details"], Value::Null);
}

/// The bytes of a one-pixel PNG.
fn base64_png() -> Vec<u8> {
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    STANDARD
        .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==")
        .unwrap()
}
