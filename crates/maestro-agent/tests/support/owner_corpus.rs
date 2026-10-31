//! Replay complete recorded owned-run inputs through the public Agent.
use super::{assistant, observations, owned, ready};
use maestro_agent::{
    Agent, AgentInitialState, AgentMessage, AgentOptions, AgentTool, CustomAgentMessages,
    PromptInput,
};
use maestro_models::{
    AssistantContent, AssistantMessageEvent, AssistantMessageEventStream, StopReason, TextContent,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

/// Typed fixture boundary rejects unused top-level members.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Every supplied public operand.
    input: Input,
    /// Complete recorded observation tree.
    expected: Value,
}
/// Fixture operands used by prompt construction, response generation and listener failure selection.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Prompt overload.
    kind: String,
    /// Text operand.
    text: Option<String>,
    /// Image operands.
    images: Option<Vec<maestro_models::ImageContent>>,
    /// Single entry operand.
    message: Option<Value>,
    /// Batch operand.
    messages: Option<Vec<Value>>,
    /// Initial conversation entries.
    history: Option<Vec<Value>>,
    /// Thinking selection.
    thinking: Option<maestro_agent::ThinkingLevel>,
    /// Stream update selection.
    updates: Option<bool>,
    /// Response stop outcome.
    stop: Option<StopReason>,
    /// Response diagnostic.
    error_message: Option<String>,
    /// Failure phase.
    fail: Option<String>,
    /// Synthetic end failure.
    second_failure: Option<bool>,
    /// Abort during stream failure.
    abort: Option<bool>,
    /// Replace model during stream failure.
    replace_model: Option<bool>,
    /// Original callback diagnostic, including empty text.
    failure_message: Option<String>,
    /// Ordered tool identifiers.
    tools: Option<Vec<String>>,
}
/// Custom message surviving only Agent history.
struct Notice(Value);
impl CustomAgentMessages for Notice {
    fn role(&self) -> &'static str {
        "notice"
    }
}
/// Decode supplied shared entries without changing authored timestamps.
fn entry(value: Value) -> AgentMessage<Notice> {
    match value["role"].as_str().unwrap() {
        "user" => AgentMessage::User(Arc::new(RwLock::new(
            serde_json::from_value(value).unwrap(),
        ))),
        "assistant" => AgentMessage::Assistant(Arc::new(RwLock::new(
            serde_json::from_value(value).unwrap(),
        ))),
        "toolResult" => AgentMessage::ToolResult(Arc::new(RwLock::new(
            serde_json::from_value(value).unwrap(),
        ))),
        "notice" => AgentMessage::Custom(Arc::new(RwLock::new(Notice(value)))),
        _ => panic!("unsupported recorded role"),
    }
}
/// Preserve all supplied fields in the public entry observation.
fn message(value: &AgentMessage<Notice>) -> Value {
    match value {
        AgentMessage::User(value) => json!(&*value.read().unwrap()),
        AgentMessage::Assistant(value) => json!(&*value.read().unwrap()),
        AgentMessage::ToolResult(value) => json!(&*value.read().unwrap()),
        AgentMessage::Custom(value) => value.read().unwrap().0.clone(),
    }
}
/// Complete public runtime observation with absent optional slots omitted.
fn state(agent: &Agent<Notice>) -> Value {
    let state = agent.state().read().unwrap();
    let mut value = json!({"messages":state.messages().read().unwrap().iter().map(message).collect::<Vec<_>>(),
        "isStreaming":state.is_streaming(), "pendingToolCalls":state.pending_tool_calls()});
    if let Some(partial) = state.streaming_message() {
        value["streamingMessage"] = message(partial);
    }
    if let Some(error) = state.error_message() {
        value["errorMessage"] = json!(error);
    }
    value
}
/// Remove only generated clock fields; retained authored timestamps remain compared.
fn normalize(value: Value) -> Value {
    match value {
        Value::Array(values) => Value::Array(values.into_iter().map(normalize).collect()),
        Value::Object(values) => Value::Object(
            values
                .into_iter()
                .filter(|(key, value)| {
                    !(key == "timestamp"
                        && value
                            .as_f64()
                            .is_some_and(|time| time > 1_000_000_000_000.0))
                })
                .map(|(key, value)| (key, normalize(value)))
                .collect(),
        ),
        Value::Number(number) => json!(number.as_f64().unwrap()),
        value => value,
    }
}
/// Replay selected inputs and compare every retained expected field.
///
/// # Panics
/// Panics if fixture inputs cannot be decoded or public observations differ.
pub async fn replay(select: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/owner_cases.json")).unwrap();
    let mut count = 0;
    for (index, case) in cases.into_iter().enumerate() {
        let selected = match select {
            "failures" => case.input.fail.is_some(),
            "reducers" => case.input.fail.is_none() && case.input.tools.is_none(),
            "pending" => case.input.tools.is_some(),
            _ => panic!("unknown replay family"),
        };
        if selected {
            count += 1;
            let actual = execute(Arc::new(case.input)).await;
            assert_eq!(
                normalize(actual),
                normalize(case.expected),
                "recorded case {index}"
            );
        }
    }
    assert!(count > 0);
}
/// Drive one submitted task and capture complete same-operation observations.
async fn execute(input: Arc<Input>) -> Value {
    let tasks = Arc::new(owned::Tasks::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let tools = input.tools.clone().unwrap_or_else(|| {
        if input.fail.as_deref().is_some_and(|phase| {
            phase.starts_with("tool_execution") || phase.starts_with("toolResult_")
        }) {
            vec!["call".into()]
        } else {
            vec![]
        }
    });
    let mut model = super::model();
    model.id = "captured".into();
    model.input = vec![maestro_models::ModelInput::Text];
    let agent: Arc<Agent<Notice>> = Arc::new(Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            model: Some(Arc::new(RwLock::new(model))),
            messages: input
                .history
                .clone()
                .unwrap_or_default()
                .into_iter()
                .map(entry)
                .collect(),
            thinking_level: input.thinking.clone(),
            tools: if tools.is_empty() {
                vec![]
            } else {
                vec![tool(&input)]
            },
            ..Default::default()
        },
        spawn: Some(tasks.spawner()),
        ..Default::default()
    }));
    install_stream(&agent, input.clone(), requests.clone(), tools);
    install_listener(&agent, input.clone(), events.clone());
    let prompt = prompt(&input);
    let observe = agent.prompt(prompt);
    tasks.take().await;
    let outcome = observe.await;
    let mut value = json!({"events":*events.lock().unwrap(),"requests":*requests.lock().unwrap(),"final":state(&agent),"signalAbsent":agent.signal().is_none()});
    if let Err(error) = outcome {
        value["error"] = json!(error.message);
    }
    value
}
/// Install listener failure selection after recording the reduced state.
fn install_listener(agent: &Arc<Agent<Notice>>, input: Arc<Input>, events: Arc<Mutex<Vec<Value>>>) {
    let owner = Arc::downgrade(agent);
    let failed = Arc::new(Mutex::new(false));
    agent.subscribe(Arc::new(move |value, _| {
        let agent = owner.upgrade().unwrap();
        let value = observations::event(&value.read().unwrap(), message);
        events.lock().unwrap().push(
            json!({"event":value,"state":state(&agent),"signalSame":agent.signal().is_some()}),
        );
        let phase = match value["type"].as_str().unwrap() {
            "message_start" => format!("{}_start", value["message"]["role"].as_str().unwrap()),
            "message_end" => format!("{}_end", value["message"]["role"].as_str().unwrap()),
            phase => phase.into(),
        };
        let mut failed = failed.lock().unwrap();
        let result = if input.second_failure == Some(true) && phase == "agent_end" {
            Err(owned::failure("listener rejected"))
        } else if input.fail.as_deref() == Some(&phase) && !*failed {
            *failed = true;
            Err(owned::failure(
                input
                    .failure_message
                    .as_deref()
                    .unwrap_or("original\nerror"),
            ))
        } else {
            Ok(())
        };
        ready(result)
    }));
}
/// Controlled echo artifact with optional source progress notification.
fn tool(input: &Arc<Input>) -> maestro_agent::SharedAgentTool {
    let input = input.clone();
    Arc::new(RwLock::new(AgentTool {
        description: None,
        definition: maestro_models::Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: json!({"type":"object","properties":{}}).into(),
        },
        label: "Echo".into(),
        prepare_arguments: None,
        execution_mode: None,
        execute: Arc::new(move |_, _, _, update| {
            if input.fail.as_deref() == Some("tool_execution_update") {
                update.unwrap()(maestro_agent::AgentToolResult {
                    content: vec![maestro_models::UserBlock::Text(TextContent::new(
                        "partial".into(),
                    ))],
                    details: Some(json!({"partial":true})),
                    terminate: None,
                    usage: None,
                });
            }
            ready(Ok(maestro_agent::AgentToolResult {
                content: vec![maestro_models::UserBlock::Text(TextContent::new(
                    "result".into(),
                ))],
                details: Some(json!({"result":true})),
                terminate: Some(true),
                usage: None,
            }))
        }),
    }))
}
/// Capture request selection and provide recorded response events.
fn install_stream(
    agent: &Arc<Agent<Notice>>,
    input: Arc<Input>,
    requests: Arc<Mutex<Vec<Value>>>,
    tools: Vec<String>,
) {
    let owner = Arc::downgrade(agent);
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |model, context, options, open| {
        let agent = owner.upgrade().unwrap();
        let mut projected = json!({"transport":options.common.transport});
        if let Some(mode) = open.extra.get("toolExecution") {
            projected["toolExecution"] = mode.clone();
        }
        if let Some(reasoning) = options.reasoning {
            projected["reasoning"] = json!(reasoning);
        }
        requests.lock().unwrap().push(json!({"model":model,"context":context,"options":projected,"signalSame":options.common.signal.is_some() && agent.signal().is_some()}));
        if input.fail.as_deref() == Some("stream") {
            if input.replace_model == Some(true) {
                let mut model = super::model();
                model.id = "failure-model".into();
                model.api = "failure-api".into();
                model.provider = "failure-provider".into();
                agent.state().write().unwrap().model = Arc::new(RwLock::new(model));
            }
            if input.abort == Some(true) {
                agent.abort();
            }
            return ready(Err(owned::failure(
                input
                    .failure_message
                    .as_deref()
                    .unwrap_or("original\nerror"),
            )));
        }
        let response = assistant("done");
        {
            let mut response = response.write().unwrap();
            response.stop_reason = input.stop.clone().unwrap_or(StopReason::Stop);
            response.error_message.clone_from(&input.error_message);
            if !tools.is_empty() {
                response.content = tools
                    .iter()
                    .map(|id| AssistantContent::ToolCall(call(id)))
                    .collect();
                response.stop_reason = StopReason::ToolUse;
            }
        }
        let stream = AssistantMessageEventStream::new();
        if input.updates == Some(true) || input.fail.as_deref() == Some("message_update") {
            updates(&stream);
        }
        stream.end(Some(response));
        ready(Ok(stream))
    }));
}
/// Nine admitted update variants with distinguishable content.
fn updates(stream: &AssistantMessageEventStream) {
    stream.push(AssistantMessageEvent::Start {
        partial: assistant("start"),
    });
    for i in 0..9 {
        let partial = assistant("done");
        let content = format!("update-{i}");
        let call = maestro_models::ToolCall {
            id: "preview".into(),
            name: "echo".into(),
            arguments: maestro_models::JsonObject::new().into(),
            thought_signature: None,
        };
        partial.write().unwrap().content = vec![match i {
            3..=5 => AssistantContent::Thinking(maestro_models::ThinkingContent {
                thinking: content.clone(),
                thinking_signature: None,
                redacted: None,
            }),
            6..=8 => AssistantContent::ToolCall(call.clone()),
            _ => AssistantContent::Text(TextContent::new(content.clone())),
        }];
        stream.push(update(i, partial, content, call));
    }
}
/// Construct one model update without changing its recorded variant fields.
fn update(
    i: usize,
    partial: maestro_models::SharedAssistantMessage,
    content: String,
    call: maestro_models::ToolCall,
) -> AssistantMessageEvent {
    let delta = format!("delta-{i}");
    match i {
        0 => AssistantMessageEvent::TextStart {
            content_index: 0,
            partial,
        },
        1 => AssistantMessageEvent::TextDelta {
            content_index: 0,
            delta,
            partial,
        },
        2 => AssistantMessageEvent::TextEnd {
            content_index: 0,
            content,
            partial,
        },
        3 => AssistantMessageEvent::ThinkingStart {
            content_index: 0,
            partial,
        },
        4 => AssistantMessageEvent::ThinkingDelta {
            content_index: 0,
            delta,
            partial,
        },
        5 => AssistantMessageEvent::ThinkingEnd {
            content_index: 0,
            content,
            partial,
        },
        6 => AssistantMessageEvent::ToolcallStart {
            content_index: 0,
            partial,
        },
        7 => AssistantMessageEvent::ToolcallDelta {
            content_index: 0,
            delta,
            partial,
        },
        8 => AssistantMessageEvent::ToolcallEnd {
            content_index: 0,
            tool_call: call,
            partial,
        },
        _ => unreachable!(),
    }
}

/// Decode the selected public prompt overload.
fn prompt(input: &Input) -> PromptInput<Notice> {
    match input.kind.as_str() {
        "text" => PromptInput::Text {
            text: input.text.clone().unwrap(),
            images: input.images.clone().unwrap_or_default(),
        },
        "message" => PromptInput::Message(entry(input.message.clone().unwrap())),
        "batch" => PromptInput::Messages(
            input
                .messages
                .clone()
                .unwrap()
                .into_iter()
                .map(entry)
                .collect(),
        ),
        _ => panic!("unknown prompt kind"),
    }
}

/// Construct a controlled echo call with literal identity.
fn call(id: &str) -> maestro_models::ToolCall {
    maestro_models::ToolCall {
        id: id.into(),
        name: "echo".into(),
        arguments: maestro_models::JsonObject::new().into(),
        thought_signature: None,
    }
}
