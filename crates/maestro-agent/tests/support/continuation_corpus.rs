//! Replay recorded observations through public conversation operations.
use super::{assistant, owned, ready, typed_user as user};
use maestro_agent::{
    Agent, AgentEvent, AgentInitialState, AgentMessage, AgentOptions, AgentTool,
    CustomAgentMessages, PromptInput, QueueMode, ToolExecutionMode,
};
use maestro_models::{
    AssistantContent, AssistantMessageEventStream, Message, StopReason, TextContent,
    ThinkingBudgets, UserBlock, UserContent, UserMessage,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};

/// Strict recorded query.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    input: Input,
    expected: Value,
}
/// All recorded operands participate in public operations.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    kind: String,
    tail: String,
    queue: String,
    mode: String,
    enqueue_at: Option<String>,
    next_mode: Option<String>,
    stop: Option<StopReason>,
    tools: Option<bool>,
    tool_execution: Option<String>,
    clear: Option<String>,
    count: Option<usize>,
    follow_at: Option<String>,
    mode_at_turn: Option<String>,
    options: Option<Options>,
    pipeline: Option<bool>,
    fail: Option<String>,
}
/// Numeric tags preserve the recorded floating-point input domain.
#[derive(Deserialize)]
#[serde(untagged)]
enum Number {
    Finite(f64),
    Tagged(String),
}
impl Number {
    /// Recover native values, not a product format.
    fn value(&self) -> f64 {
        match self {
            Self::Finite(v) => *v,
            Self::Tagged(v) => match v.as_str() {
                "-0" => -0.0,
                "NaN" => f64::NAN,
                "Infinity" => f64::INFINITY,
                "-Infinity" => f64::NEG_INFINITY,
                _ => panic!("unknown numeric tag"),
            },
        }
    }
}
/// Strict optional budget fields.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Budgets {
    minimal: Option<Number>,
    low: Option<Number>,
    medium: Option<Number>,
    high: Option<Number>,
}
impl Budgets {
    /// Recover each declared level.
    fn value(&self) -> ThinkingBudgets {
        ThinkingBudgets {
            minimal: self.minimal.as_ref().map(Number::value),
            low: self.low.as_ref().map(Number::value),
            medium: self.medium.as_ref().map(Number::value),
            high: self.high.as_ref().map(Number::value),
        }
    }
}
/// Strict scalar configuration operands.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Options {
    transport: Option<maestro_models::Transport>,
    session_id: Option<String>,
    max_retry_delay_ms: Option<Number>,
    thinking_budgets: Option<Budgets>,
}
/// Custom history role converted only by the configured callback.
struct Notice(String);
impl CustomAgentMessages for Notice {
    fn role(&self) -> &'static str {
        "notice"
    }
}
/// Projection used by both generated history and model requests.
fn record(value: &AgentMessage<Notice>) -> Value {
    match value {
        AgentMessage::User(v) => record_model(&Message::User(v.read().unwrap().clone())),
        AgentMessage::Assistant(v) => record_model(&Message::Assistant(v.read().unwrap().clone())),
        AgentMessage::ToolResult(v) => {
            record_model(&Message::ToolResult(v.read().unwrap().clone()))
        }
        AgentMessage::Custom(v) => json!({"role":"notice","text":v.read().unwrap().0}),
    }
}
/// Preserve ordered content in the recorded display projection.
fn record_model(value: &Message) -> Value {
    let value = serde_json::to_value(value).unwrap();
    let text = match &value["content"] {
        Value::String(v) => v.clone(),
        Value::Array(v) => v
            .iter()
            .map(|v| {
                if v["type"] == "toolCall" {
                    format!("call:{}", v["id"].as_str().unwrap())
                } else {
                    v["text"].as_str().unwrap_or("").into()
                }
            })
            .collect::<Vec<_>>()
            .join("|"),
        _ => panic!("missing model content"),
    };
    json!({"role":value["role"],"text":text})
}
/// Retain sign and nonfinite values in fixture-only encoding.
fn numeric(v: f64) -> Value {
    if v.is_nan() {
        json!("NaN")
    } else if v == f64::INFINITY {
        json!("Infinity")
    } else if v == f64::NEG_INFINITY {
        json!("-Infinity")
    } else if v == 0.0 && v.is_sign_negative() {
        json!("-0")
    } else {
        json!(v)
    }
}
/// Preserve absent levels instead of inventing null members.
fn budgets(v: &ThinkingBudgets) -> Value {
    let mut value = json!({});
    for (name, field) in [
        ("minimal", v.minimal),
        ("low", v.low),
        ("medium", v.medium),
        ("high", v.high),
    ] {
        if let Some(field) = field {
            value[name] = numeric(field);
        }
    }
    value
}
/// Decode the supplied drain policy.
fn mode(value: &str) -> QueueMode {
    match value {
        "all" => QueueMode::All,
        "one-at-a-time" => QueueMode::OneAtATime,
        _ => panic!("unknown queue mode"),
    }
}
/// Return the canonical event name and recorded message/call suffix.
fn event(value: &AgentEvent<Notice>) -> (String, &'static str) {
    let (name, suffix) = match value {
        AgentEvent::AgentStart => ("agent_start", None),
        AgentEvent::AgentEnd { .. } => ("agent_end", None),
        AgentEvent::TurnStart => ("turn_start", None),
        AgentEvent::TurnEnd { .. } => ("turn_end", Some("assistant".into())),
        AgentEvent::MessageStart { message } => (
            "message_start",
            Some(record(message)["role"].as_str().unwrap().into()),
        ),
        AgentEvent::MessageEnd { message } => (
            "message_end",
            Some(record(message)["role"].as_str().unwrap().into()),
        ),
        AgentEvent::MessageUpdate { .. } => ("message_update", Some("assistant".into())),
        AgentEvent::ToolExecutionStart { tool_call } => {
            ("tool_execution_start", Some(tool_call.id.clone()))
        }
        AgentEvent::ToolExecutionEnd { tool_call, .. } => {
            ("tool_execution_end", Some(tool_call.id.clone()))
        }
        AgentEvent::ToolExecutionUpdate { tool_call, .. } => {
            ("tool_execution_update", Some(tool_call.id.clone()))
        }
    };
    (
        suffix.map_or_else(|| name.into(), |suffix| format!("{name}:{suffix}")),
        name,
    )
}
/// Select a disjoint primary behavior owner for every query.
fn family(input: &Input) -> &'static str {
    if input.fail.is_some() {
        "R6"
    } else if input.pipeline == Some(true) {
        "R4"
    } else if input.options.is_some() {
        "R3"
    } else if input.enqueue_at.is_some() {
        if input.kind == "continue" { "C5" } else { "Q3" }
    } else if input.next_mode.is_some() || input.mode_at_turn.is_some() {
        "Q2"
    } else if input.clear.is_some() {
        "Q8"
    } else if input.stop.is_some() {
        "Q7"
    } else if input.tools == Some(true) {
        "Q4"
    } else if input.follow_at.is_some() {
        "Q5"
    } else if input.tail == "empty" || (input.tail == "assistant" && input.queue == "none") {
        "C1"
    } else if input.tail != "assistant" {
        "C2"
    } else if input.queue == "steering" {
        "C4"
    } else {
        "C3"
    }
}
/// Replay each selected full expected tree, with no ignored observation fields.
///
/// # Panics
/// Panics on malformed fixtures or behavioral disagreement.
pub async fn replay(selected: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/continuation_cases.json")).unwrap();
    let mut count = 0;
    for (index, case) in cases.into_iter().enumerate() {
        if family(&case.input) == selected {
            count += 1;
            let actual = execute(Arc::new(case.input)).await;
            assert!(
                normalize(actual) == normalize(case.expected),
                "recorded query {index}"
            );
        }
    }
    assert!(count > 0, "no queries for family");
}
/// Compare reference numbers as f64 while preserving every key and ordered array entry.
fn normalize(value: Value) -> Value {
    match value {
        Value::Number(v) => json!(v.as_f64().unwrap()),
        Value::Array(v) => Value::Array(v.into_iter().map(normalize).collect()),
        Value::Object(v) => Value::Object(v.into_iter().map(|(k, v)| (k, normalize(v))).collect()),
        v => v,
    }
}
/// Capture effect order and return the caller's exact multiline diagnostic.
fn phase(
    input: &Input,
    trace: &Mutex<Vec<String>>,
    name: &str,
) -> Result<(), maestro_models::DiagnosticErrorInfo> {
    trace.lock().unwrap().push(name.into());
    if input.fail.as_deref() == Some(name) {
        Err(owned::failure(&format!("{name}\nfailed")))
    } else {
        Ok(())
    }
}
/// Initial history for the selected tail branch.
fn history(tail: &str) -> Vec<AgentMessage<Notice>> {
    match tail {
        "empty" => vec![], "user" => vec![user("old-user")],
        "assistant" => vec![user("old-user"), AgentMessage::Assistant(assistant("old-answer"))],
        "custom" => vec![AgentMessage::Custom(Arc::new(RwLock::new(Notice("old-notice".into()))))],
        "toolResult" => vec![AgentMessage::ToolResult(Arc::new(RwLock::new(serde_json::from_value(json!({"role":"toolResult","toolCallId":"old-call","toolName":"echo","content":[{"type":"text","text":"old-result"}],"isError":false,"timestamp":3})).unwrap())))],
        _ => panic!("unknown history tail"),
    }
}
/// Construct the exact recorded optional configuration.
fn configured(input: &Input, tasks: &Arc<owned::Tasks>) -> Arc<Agent<Notice>> {
    let options = input.options.as_ref();
    let mut model = super::model();
    model.id = "selected".into();
    Arc::new(Agent::new(AgentOptions {
        initial_state: AgentInitialState {
            model: Some(Arc::new(RwLock::new(model))),
            messages: history(&input.tail),
            ..Default::default()
        },
        spawn: Some(tasks.spawner()),
        steering_mode: Some(mode(&input.mode)),
        follow_up_mode: Some(mode(&input.mode)),
        session_id: options.and_then(|v| v.session_id.clone()),
        transport: options.and_then(|v| v.transport.clone()),
        max_retry_delay_ms: options.and_then(|v| v.max_retry_delay_ms.as_ref().map(Number::value)),
        thinking_budgets: options
            .and_then(|v| v.thinking_budgets.as_ref())
            .map(|v| Arc::new(RwLock::new(v.value()))),
        tool_execution: input.tool_execution.as_ref().map(|v| {
            if v == "sequential" {
                ToolExecutionMode::Sequential
            } else {
                assert_eq!(v, "parallel");
                ToolExecutionMode::Parallel
            }
        }),
        ..Default::default()
    }))
}
/// Execute one public invocation; task settlement closes its observations.
async fn execute(input: Arc<Input>) -> Value {
    let tasks = Arc::new(owned::Tasks::default());
    let events = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let trace = Arc::new(Mutex::new(Vec::new()));
    let agent = configured(&input, &tasks);
    install_pipeline(&agent, &input, &trace);
    install_tools(&agent, &input, &trace);
    install_stream(&agent, &input, &trace, &requests);
    install_listener(&agent, &input, &events);
    enqueue(&agent, &input);
    let observe = if input.kind == "prompt" {
        agent.prompt(PromptInput::Message(user("prompt")))
    } else {
        assert_eq!(input.kind, "continue");
        agent.r#continue()
    };
    let task = tasks.0.lock().unwrap().pop();
    if let Some(task) = task {
        task.await;
    }
    let outcome = observe.await;
    let state = agent.state().read().unwrap();
    let mut actual = json!({"requests":*requests.lock().unwrap(),"events":*events.lock().unwrap(),"trace":*trace.lock().unwrap(),"history":state.messages().read().unwrap().iter().map(record).collect::<Vec<_>>(),"queued":agent.has_queued_messages(),"streaming":state.is_streaming(),"signalAbsent":agent.signal().is_none()});
    if let Some(error) = state.error_message() {
        actual["stateError"] = json!(error);
    }
    if let Err(error) = outcome {
        actual["failure"] = json!(error.message);
    }
    actual
}
/// Apply recorded enqueue, policy replacement and clear operands.
fn enqueue(agent: &Agent<Notice>, input: &Input) {
    for text in if input.count == Some(1) {
        vec!["only"]
    } else {
        assert!(input.count.is_none());
        vec!["zulu", "alpha", "zulu"]
    } {
        if input.queue == "steering" || input.queue == "both" {
            agent.steer(user(&format!("steer:{text}")));
        }
        if input.queue == "follow" || input.queue == "both" {
            agent.follow_up(user(&format!("follow:{text}")));
        }
    }
    if let Some(next) = &input.next_mode {
        agent.set_steering_mode(mode(next));
        agent.set_follow_up_mode(mode(next));
    }
    match input.clear.as_deref() {
        Some("steering") => agent.clear_steering_queue(),
        Some("follow") => agent.clear_follow_up_queue(),
        Some("both") => agent.clear_all_queues(),
        None => (),
        _ => panic!("unknown clear selection"),
    }
}
/// Entry and completed-turn effects happen in the awaited listener.
fn install_listener(
    agent: &Arc<Agent<Notice>>,
    input: &Arc<Input>,
    events: &Arc<Mutex<Vec<String>>>,
) {
    let owner = Arc::downgrade(agent);
    let input = input.clone();
    let events = events.clone();
    let injected = Mutex::new(false);
    agent.subscribe(Arc::new(move |value, _| {
        let (recorded, name) = event(&value.read().unwrap());
        events.lock().unwrap().push(recorded);
        let agent = owner.upgrade().unwrap();
        let mut injected = injected.lock().unwrap();
        if !*injected {
            if input.enqueue_at.as_deref() == Some(name) {
                *injected = true;
                agent.steer(user("entry-steering"));
            }
            if input.follow_at.as_deref() == Some(name) {
                *injected = true;
                agent.follow_up(user("late-follow"));
            }
            if let Some(next) = &input.mode_at_turn
                && name == "turn_end"
            {
                *injected = true;
                agent.set_steering_mode(mode(next));
                agent.set_follow_up_mode(mode(next));
            }
        }
        ready(Ok(()))
    }));
}
/// Install the transformed custom records and per-turn credentials.
fn install_pipeline(
    agent: &Arc<Agent<Notice>>,
    input: &Arc<Input>,
    trace: &Arc<Mutex<Vec<String>>>,
) {
    if input.pipeline != Some(true) {
        return;
    }
    let owner = Arc::downgrade(agent);
    let i = input.clone();
    let t = trace.clone();
    *agent.transform_context.write().unwrap() = Some(Arc::new(move |history, signal| {
        assert_eq!(
            signal.is_some(),
            owner.upgrade().unwrap().signal().is_some()
        );
        let result = phase(&i, &t, "transform").map(|()| {
            let mut entries = history.read().unwrap().clone();
            entries.push(AgentMessage::Custom(Arc::new(RwLock::new(Notice(
                "transformed".into(),
            )))));
            Arc::new(RwLock::new(entries))
        });
        ready(result)
    }));
    let i = input.clone();
    let t = trace.clone();
    *agent.convert_to_llm.write().unwrap() = Arc::new(move |history| {
        let result = phase(&i, &t, "convert").map(|()| {
            history
                .read()
                .unwrap()
                .iter()
                .map(|v| match v {
                    AgentMessage::Custom(v) => Message::User(UserMessage {
                        content: UserContent::Text(format!("converted:{}", v.read().unwrap().0)),
                        timestamp: 1.0,
                    }),
                    AgentMessage::User(v) => Message::User(v.read().unwrap().clone()),
                    AgentMessage::Assistant(v) => Message::Assistant(v.read().unwrap().clone()),
                    AgentMessage::ToolResult(v) => Message::ToolResult(v.read().unwrap().clone()),
                })
                .collect()
        });
        ready(result)
    });
    let i = input.clone();
    let t = trace.clone();
    let keys = Mutex::new(0_usize);
    *agent.get_api_key.write().unwrap() = Some(Arc::new(move |_| {
        let result = phase(&i, &t, "key").map(|()| {
            let mut n = keys.lock().unwrap();
            let value = match *n % 4 {
                0 => Some("first-key".into()),
                1 => Some(String::new()),
                2 => None,
                _ => Some("fourth-key".into()),
            };
            *n += 1;
            value
        });
        ready(result)
    }));
}
/// Existing hook catch paths and tool-triggered steering.
fn install_tools(agent: &Arc<Agent<Notice>>, input: &Arc<Input>, trace: &Arc<Mutex<Vec<String>>>) {
    if input.tools == Some(true) {
        let owner = Arc::downgrade(agent);
        let t = trace.clone();
        let tool = Arc::new(RwLock::new(AgentTool {
            description: None,
            definition: maestro_models::Tool {
                name: "echo".into(),
                description: "Echo".into(),
                parameters: json!({"type":"object","properties":{}}).into(),
            },
            label: "Echo".into(),
            prepare_arguments: None,
            execution_mode: None,
            execute: Arc::new(move |id, _, _, _| {
                t.lock().unwrap().push(format!("execute:{id}"));
                owner
                    .upgrade()
                    .unwrap()
                    .steer(user(&format!("during:{id}")));
                ready(Ok(maestro_agent::AgentToolResult {
                    content: vec![UserBlock::Text(TextContent::new(format!("result:{id}")))],
                    details: Some(json!({})),
                    terminate: None,
                    usage: None,
                }))
            }),
        }));
        agent.state().write().unwrap().set_tools(vec![tool]);
    }
    if input.fail.as_deref() == Some("before") {
        let i = input.clone();
        let t = trace.clone();
        *agent.before_tool_call.write().unwrap() = Some(Arc::new(move |_, _| {
            ready(phase(&i, &t, "before").map(|()| None))
        }));
    }
    if input.fail.as_deref() == Some("after") {
        let i = input.clone();
        let t = trace.clone();
        *agent.after_tool_call.write().unwrap() = Some(Arc::new(move |_, _| {
            ready(phase(&i, &t, "after").map(|()| None))
        }));
    }
    if input.fail.as_deref() == Some("payload") {
        let i = input.clone();
        let t = trace.clone();
        *agent.on_payload.write().unwrap() = Some(Arc::new(move |p, _| {
            ready(phase(&i, &t, "payload").map(|()| p))
        }));
    }
    if input.fail.as_deref() == Some("response") {
        let i = input.clone();
        let t = trace.clone();
        *agent.on_response.write().unwrap() =
            Some(Arc::new(move |_, _| ready(phase(&i, &t, "response"))));
    }
}
/// Controlled stream records all options before any selected hook failure.
fn install_stream(
    agent: &Arc<Agent<Notice>>,
    input: &Arc<Input>,
    trace: &Arc<Mutex<Vec<String>>>,
    requests: &Arc<Mutex<Vec<Value>>>,
) {
    let input = input.clone();
    let trace = trace.clone();
    let requests = requests.clone();
    let turn = Mutex::new(0_usize);
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |model, context, options, open| {
        phase(&input, &trace, "stream").unwrap();
        let turn = {
            let mut turn = turn.lock().unwrap();
            *turn += 1;
            *turn
        };
        let mut projected = json!({"transport":options.common.transport,"toolExecution":open.extra["toolExecution"]});
        if let Some(v) = &options.common.session_id {
            projected["sessionId"] = json!(v);
        }
        if let Some(v) = &options.common.api_key {
            projected["apiKey"] = json!(v);
        }
        if let Some(v) = &options.thinking_budgets {
            projected["thinkingBudgets"] = budgets(v);
        }
        if let Some(v) = options.common.max_retry_delay_ms {
            projected["maxRetryDelayMs"] = numeric(v);
        }
        requests.lock().unwrap().push(json!({"messages":context.messages.iter().map(record_model).collect::<Vec<_>>(),"model":{"id":model.id,"provider":model.provider},"options":projected}));
        let input = input.clone();
        Box::pin(async move {
            let model = Arc::new(model);
            if input.fail.as_deref() == Some("payload") {
                options.common.on_payload.unwrap()(
                    maestro_models::PayloadHookInput {
                        payload: json!({"original":true}),
                        signal: None,
                    },
                    model.clone(),
                )
                .await?;
            }
            if input.fail.as_deref() == Some("response") {
                options.common.on_response.unwrap()(
                    maestro_models::ProviderResponse {
                        status: 200.0,
                        headers: std::collections::BTreeMap::default(),
                    },
                    model,
                )
                .await?;
            }
            let result = response(&input, turn);
            let stream = AssistantMessageEventStream::new();
            stream.end(Some(result));
            Ok(stream)
        })
    }));
}

/// Produce a recorded terminal response or the first two-call batch.
fn response(input: &Input, turn: usize) -> maestro_models::SharedAssistantMessage {
    let result = assistant(&format!("answer-{turn}"));
    if let Some(stop) = &input.stop {
        result.write().unwrap().stop_reason = stop.clone();
    }
    if input.tools == Some(true) && turn == 1 {
        let mut result = result.write().unwrap();
        result.stop_reason = StopReason::ToolUse;
        result.content = ["zulu", "alpha"]
            .into_iter()
            .map(|id| {
                AssistantContent::ToolCall(maestro_models::ToolCall {
                    id: id.into(),
                    name: "echo".into(),
                    arguments: maestro_models::JsonObject::new().into(),
                    thought_signature: None,
                })
            })
            .collect();
    }
    result
}
