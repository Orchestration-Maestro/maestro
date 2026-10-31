//! Captured configuration and controlled native readiness observations.
use super::{assistant, owned, ready};
use maestro_agent::{Agent, AgentEvent, AgentMessage};
use maestro_models::{AssistantMessageEventStream, StopReason};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
/// Every recorded input is selected by its named native scenario.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Input {
    /// Scenario name.
    case: String,
    /// Public operation selection.
    operation: Option<String>,
    /// Reset history during a busy continuation.
    reset_history: Option<bool>,
    /// Active control operation.
    action: Option<String>,
}
/// Complete expected observation tree.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Consumed public operands.
    input: Input,
    /// Full public observation.
    expected: Value,
}
/// Compare complete observations for a named scenario.
///
/// # Panics
/// Panics on malformed fixture input or an observation mismatch.
pub fn compare(name: &str, operation: Option<&str>, actual: Value) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/binding_cases.json")).unwrap();
    let case = cases
        .into_iter()
        .find(|v| v.input.case == name && v.input.operation.as_deref() == operation)
        .unwrap();
    assert!(case.input.reset_history.is_none() && case.input.action.is_none());
    assert!(
        normalize(actual) == normalize(case.expected),
        "binding scenario {name}"
    );
}
/// Compare numeric reference records without changing any retained dimension.
fn normalize(v: Value) -> Value {
    match v {
        Value::Number(v) => json!(v.as_f64().unwrap()),
        Value::Array(v) => Value::Array(v.into_iter().map(normalize).collect()),
        Value::Object(v) => Value::Object(v.into_iter().map(|(k, v)| (k, normalize(v))).collect()),
        v => v,
    }
}
/// Plain event name for lifecycle ordering.
fn event(v: &AgentEvent) -> &'static str {
    match v {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageEnd { .. } => "message_end",
        _ => panic!("unexpected control event"),
    }
}
/// Ordered common roles of the retained history.
fn roles(agent: &Agent) -> Vec<&'static str> {
    super::observable::roles(&agent.state().read().unwrap().messages().read().unwrap())
}
/// Pause the first credential request without retaining an internal guard.
fn key_gate(
    agent: &Agent,
) -> (
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<()>,
) {
    let (entered, entry) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let gates = Mutex::new(Some((entered, released)));
    *agent.get_api_key.write().unwrap() = Some(Arc::new(move |_| {
        let gate = gates.lock().unwrap().take();
        Box::pin(async move {
            super::wait_gate(gate).await;
            Ok(None)
        })
    }));
    (release, entry)
}
/// Record plain lifecycle names and return the selected terminal outcome.
fn control_callbacks(agent: &Arc<Agent>, name: &str, events: &Arc<Mutex<Vec<&'static str>>>) {
    let logged = events.clone();
    agent.subscribe(Arc::new(move |v, _| {
        logged.lock().unwrap().push(event(&v.read().unwrap()));
        ready(Ok(()))
    }));
    let stop = if name == "active_control" {
        StopReason::Aborted
    } else {
        StopReason::Stop
    };
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, _, _, _| {
        let stream = AssistantMessageEventStream::new();
        let response = assistant("done");
        response.write().unwrap().stop_reason = stop.clone();
        stream.end(Some(response));
        ready(Ok(stream))
    }));
}
/// Drive gated credential work and join its actual submitted task.
///
/// # Panics
/// Panics on malformed fixtures or public operation disagreement.
pub async fn controls(name: &str) {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("../fixtures/binding_cases.json")).unwrap();
    for case in cases.into_iter().filter(|v| v.input.case == name) {
        control_case(name, case).await;
    }
}

/// Observe one paused active operation through its recorded control operand.
async fn control_case(name: &str, case: Case) {
    assert!(case.input.operation.is_none());
    let (agent, tasks) = owned::agent();
    let (release, entry) = key_gate(&agent);
    let events = Arc::new(Mutex::new(Vec::new()));
    control_callbacks(&agent, name, &events);
    let observe = agent.prompt(owned::input("first"));
    let joined = tokio::spawn(tasks.take());
    entry.await.unwrap();
    let actual = if name == "busy_continue" {
        assert!(case.input.action.is_none());
        let (error, queued) = busy_continuation(&agent, case.input.reset_history.unwrap()).await;
        release.send(()).unwrap();
        joined.await.unwrap();
        observe.await.unwrap();
        json!({"error":error,"queuedWhileBusy":queued,"events":*events.lock().unwrap(),"history":roles(&agent)})
    } else {
        assert_eq!(name, "active_control");
        assert!(case.input.reset_history.is_none());
        let immediate = apply_control(&agent, case.input.action.as_deref().unwrap());
        assert_eq!(
            agent.r#continue().await.unwrap_err().message,
            "Agent is already processing. Wait for completion before continuing."
        );
        release.send(()).unwrap();
        joined.await.unwrap();
        observe.await.unwrap();
        json!({"immediate":immediate,"queuedAfter":agent.has_queued_messages(),"roles":roles(&agent)})
    };
    assert!(actual == case.expected, "control scenario {name}");
}
/// Tagged callback configuration captured independently of its live slots.
fn tagged(
    agent: &Arc<Agent>,
    tag: &'static str,
    trace: &Arc<Mutex<Vec<String>>>,
    requests: &Arc<Mutex<Vec<Value>>>,
    calls: &Arc<Mutex<usize>>,
) {
    let t = trace.clone();
    *agent.transform_context.write().unwrap() = Some(Arc::new(move |history, _| {
        t.lock().unwrap().push(format!("transform:{tag}"));
        let mut history = history.read().unwrap().clone();
        history.push(super::user(&format!("transform-{tag}")));
        ready(Ok(Arc::new(std::sync::RwLock::new(history))))
    }));
    let t = trace.clone();
    *agent.convert_to_llm.write().unwrap() = Arc::new(move |history| {
        t.lock().unwrap().push(format!("convert:{tag}"));
        ready(Ok(history
            .read()
            .unwrap()
            .iter()
            .map(|v| match v {
                AgentMessage::User(v) => {
                    let mut v = v.read().unwrap().clone();
                    let maestro_models::UserContent::Text(text) = &v.content else {
                        panic!("expected text fixture")
                    };
                    v.content = maestro_models::UserContent::Text(format!("{text}:{tag}"));
                    maestro_models::Message::User(v)
                }
                AgentMessage::Assistant(v) => {
                    maestro_models::Message::Assistant(v.read().unwrap().clone())
                }
                AgentMessage::ToolResult(v) => {
                    maestro_models::Message::ToolResult(v.read().unwrap().clone())
                }
                AgentMessage::Custom(_) => unreachable!(),
            })
            .collect()))
    });
    let t = trace.clone();
    let count = calls.clone();
    *agent.get_api_key.write().unwrap() = Some(Arc::new(move |_| {
        t.lock().unwrap().push(format!("key:{tag}"));
        ready(Ok(Some(format!("{tag}-key-{}", *count.lock().unwrap()))))
    }));
    tagged_hooks(agent, tag, trace);
    tagged_scalars(agent, tag);
    tagged_stream(agent, tag, trace, requests, calls);
}
/// Exact optional scalar values for the old and replacement configuration.
fn tagged_scalars(agent: &Agent, tag: &str) {
    *agent.session_id.write().unwrap() = Some(format!("session-{tag}"));
    *agent.thinking_budgets.write().unwrap() = Some(Arc::new(std::sync::RwLock::new(
        maestro_models::ThinkingBudgets {
            high: Some(if tag == "old" { 10.0 } else { 99.0 }),
            ..Default::default()
        },
    )));
    *agent.transport.write().unwrap() = if tag == "old" {
        maestro_models::Transport::Sse
    } else {
        maestro_models::Transport::Websocket
    };
    *agent.max_retry_delay_ms.write().unwrap() = Some(if tag == "old" { 0.5 } else { 99.0 });
    *agent.tool_execution.write().unwrap() = if tag == "old" {
        maestro_agent::ToolExecutionMode::Sequential
    } else {
        maestro_agent::ToolExecutionMode::Parallel
    };
}
/// Tagged model and tool hooks share the recorded call trace.
fn tagged_hooks(agent: &Agent, tag: &'static str, trace: &Arc<Mutex<Vec<String>>>) {
    let t = trace.clone();
    *agent.on_payload.write().unwrap() = Some(Arc::new(move |mut p, _| {
        t.lock().unwrap().push(format!("payload:{tag}"));
        p.payload["tag"] = json!(tag);
        ready(Ok(p))
    }));
    let t = trace.clone();
    *agent.on_response.write().unwrap() = Some(Arc::new(move |_, _| {
        t.lock().unwrap().push(format!("response:{tag}"));
        ready(Ok(()))
    }));
    let t = trace.clone();
    *agent.before_tool_call.write().unwrap() = Some(Arc::new(move |_, _| {
        t.lock().unwrap().push(format!("before:{tag}"));
        ready(Ok(None))
    }));
    let t = trace.clone();
    *agent.after_tool_call.write().unwrap() = Some(Arc::new(move |_, _| {
        t.lock().unwrap().push(format!("after:{tag}"));
        ready(Ok(Some(maestro_agent::AfterToolCallResult {
            terminate: Some(true),
            ..Default::default()
        })))
    }));
}
/// Controlled tagged dispatch invokes forwarded model hooks.
fn tagged_stream(
    agent: &Agent,
    tag: &'static str,
    trace: &Arc<Mutex<Vec<String>>>,
    requests: &Arc<Mutex<Vec<Value>>>,
    calls: &Arc<Mutex<usize>>,
) {
    let t = trace.clone();
    let requests = requests.clone();
    let count = calls.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |model, context, options, open| {
        let call = {
            let mut count = count.lock().unwrap();
            *count += 1;
            *count
        };
        t.lock().unwrap().push(format!("stream:{tag}"));
        let requests = requests.clone();
        Box::pin(async move {
            let model = Arc::new(model);
            let payload = options.common.on_payload.unwrap()(
                maestro_models::PayloadHookInput {
                    payload: json!({"untouched":true}),
                    signal: None,
                },
                model.clone(),
            )
            .await?;
            options.common.on_response.unwrap()(
                maestro_models::ProviderResponse {
                    status: 200.0,
                    headers: std::collections::BTreeMap::new(),
                },
                model,
            )
            .await?;
            requests.lock().unwrap().push(json!({"tag":tag,"messages":context.messages.iter().map(|v|match v {maestro_models::Message::User(v)=>{let maestro_models::UserContent::Text(v)=&v.content else {unreachable!()};v.clone()},maestro_models::Message::Assistant(_)=>"assistant".into(),maestro_models::Message::ToolResult(_)=>"toolResult".into()}).collect::<Vec<_>>(),"session":options.common.session_id,"budget":options.thinking_budgets.unwrap().high,"transport":options.common.transport,"retry":options.common.max_retry_delay_ms,"execution":open.extra["toolExecution"],"key":options.common.api_key,"payload":payload.payload}));
            let response = assistant("ok");
            response.write().unwrap().content = vec![maestro_models::AssistantContent::ToolCall(
                maestro_models::ToolCall {
                    id: format!("call-{call}"),
                    name: "echo".into(),
                    arguments: maestro_models::JsonObject::new().into(),
                    thought_signature: None,
                },
            )];
            let stream = AssistantMessageEventStream::new();
            stream.end(Some(response));
            Ok(stream)
        })
    }));
}
/// Swap every callback and scalar at entry; then remove optional fields on a later run.
///
/// # Panics
/// Panics if the recorded typed configuration is not preserved.
pub async fn captured() {
    for operation in ["prompt", "continue"] {
        captured_operation(operation).await;
    }
}
/// Drive one admitted configuration and its two replacement runs.
async fn captured_operation(operation: &str) {
    let (agent, tasks) = owned::agent();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let requests = Arc::new(Mutex::new(Vec::new()));
    let calls = Arc::new(Mutex::new(0));
    agent
        .state()
        .write()
        .unwrap()
        .set_messages(vec![super::user("history")]);
    captured_tool(&agent);
    tagged(&agent, "old", &trace, &requests, &calls);
    let owner = Arc::downgrade(&agent);
    let t = trace.clone();
    let r = requests.clone();
    let c = calls.clone();
    let swapped = Mutex::new(false);
    let subscription = agent.subscribe(Arc::new(move |v, _| {
        if matches!(&*v.read().unwrap(), AgentEvent::AgentStart) {
            let swap = {
                let mut swapped = swapped.lock().unwrap();
                let swap = !*swapped;
                *swapped = true;
                swap
            };
            if swap {
                tagged(&owner.upgrade().unwrap(), "next", &t, &r, &c);
            }
        }
        ready(Ok(()))
    }));
    let observe = if operation == "prompt" {
        agent.prompt(maestro_agent::PromptInput::Message(super::user("prompt")))
    } else {
        agent.r#continue()
    };
    tasks.take().await;
    observe.await.unwrap();
    let observe = agent.prompt(maestro_agent::PromptInput::Message(super::user(
        "next-prompt",
    )));
    tasks.take().await;
    observe.await.unwrap();
    subscription.unsubscribe();
    let removed = Arc::new(Mutex::new(Value::Null));
    remove_options(&agent, &removed);
    let observe = agent.prompt(maestro_agent::PromptInput::Message(super::user(
        "after-removal",
    )));
    tasks.take().await;
    observe.await.unwrap();
    compare(
        "captured_configuration",
        Some(operation),
        json!({"requests":*requests.lock().unwrap(),"trace":*trace.lock().unwrap(),"removed":*removed.lock().unwrap()}),
    );
}

/// Install the declared tool whose after hook terminates the captured run.
fn captured_tool(agent: &Agent) {
    let tool = super::with_execute(Arc::new(|_, _, _, _| {
        ready(Ok(maestro_agent::AgentToolResult {
            content: vec![],
            details: Some(json!({})),
            terminate: None,
            usage: None,
        }))
    }))
    .tools
    .unwrap()
    .read()
    .unwrap()[0]
        .clone();
    agent.state().write().unwrap().set_tools(vec![tool]);
}
/// Remove optional captures and record the forwarded absence projection.
fn remove_options(agent: &Agent, removed: &Arc<Mutex<Value>>) {
    let defaults: Agent = Agent::default();
    *agent.convert_to_llm.write().unwrap() = defaults.convert_to_llm.read().unwrap().clone();
    *agent.transform_context.write().unwrap() = None;
    *agent.get_api_key.write().unwrap() = None;
    *agent.on_payload.write().unwrap() = None;
    *agent.on_response.write().unwrap() = None;
    *agent.before_tool_call.write().unwrap() = None;
    *agent.after_tool_call.write().unwrap() = None;
    *agent.session_id.write().unwrap() = None;
    *agent.thinking_budgets.write().unwrap() = None;
    *agent.max_retry_delay_ms.write().unwrap() = None;
    *agent.transport.write().unwrap() = maestro_models::Transport::Auto;
    *agent.tool_execution.write().unwrap() = maestro_agent::ToolExecutionMode::Parallel;
    let recorded = removed.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, context, options, open| {
        let mut defined = Vec::new();
        for (name, present) in [
            ("sessionId", options.common.session_id.is_some()),
            ("thinkingBudgets", options.thinking_budgets.is_some()),
            (
                "maxRetryDelayMs",
                options.common.max_retry_delay_ms.is_some(),
            ),
            ("onPayload", options.common.on_payload.is_some()),
            ("onResponse", options.common.on_response.is_some()),
            ("apiKey", options.common.api_key.is_some()),
            ("transport", options.common.transport.is_some()),
            ("toolExecution", open.extra.contains_key("toolExecution")),
        ] {
            if present {
                defined.push(name);
            }
        }
        *recorded.lock().unwrap() = json!({"messages":context.messages.iter().map(|v|match v {maestro_models::Message::User(_)=>"user",maestro_models::Message::Assistant(_)=>"assistant",maestro_models::Message::ToolResult(_)=>"toolResult"}).collect::<Vec<_>>(),"defined":defined,"transport":options.common.transport,"execution":open.extra["toolExecution"]});
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(assistant("removed")));
        ready(Ok(stream))
    }));
}

/// Record provider selection before waiting and request snapshots afterward.
fn retained_requests(
    agent: &Agent,
    providers: &Arc<Mutex<Vec<String>>>,
    records: &Arc<Mutex<Vec<Value>>>,
) -> (
    tokio::sync::oneshot::Sender<()>,
    tokio::sync::oneshot::Receiver<()>,
) {
    let (entered, entry) = tokio::sync::oneshot::channel();
    let (release, released) = tokio::sync::oneshot::channel();
    let gates = Mutex::new(Some((entered, released)));
    let recorded = providers.clone();
    let requests = records.clone();
    *agent.get_api_key.write().unwrap() = Some(Arc::new(move |provider| {
        recorded.lock().unwrap().push(provider);
        let gate = gates.lock().unwrap().take();
        let turn = requests.lock().unwrap().len();
        Box::pin(async move {
            super::wait_gate(gate).await;
            Ok(Some(format!("key-{turn}")))
        })
    }));
    let recorded = records.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |model, _, options, _| {
        recorded.lock().unwrap().push(json!({"high":options.thinking_budgets.unwrap().high,"provider":model.provider,"id":model.id,"sessionId":options.common.session_id,"key":options.common.api_key}));
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(assistant("ok")));
        ready(Ok(stream))
    }));
    (release, entry)
}
/// Edits to captured objects survive both key waiting and the next turn.
///
/// # Panics
/// Panics on failed readiness handshakes or changed object identity.
pub async fn retained() {
    let (agent, tasks) = owned::agent();
    let model = Arc::new(std::sync::RwLock::new(super::model()));
    model.write().unwrap().provider = "before".into();
    agent.state().write().unwrap().model = model.clone();
    let budget = Arc::new(std::sync::RwLock::new(maestro_models::ThinkingBudgets {
        high: Some(10.0),
        ..Default::default()
    }));
    let weak = Arc::downgrade(&budget);
    *agent.thinking_budgets.write().unwrap() = Some(budget.clone());
    *agent.session_id.write().unwrap() = Some("old-session".into());
    let providers = Arc::new(Mutex::new(Vec::new()));
    let records = Arc::new(Mutex::new(Vec::new()));
    let (release, entry) = retained_requests(&agent, &providers, &records);
    let edited = Arc::downgrade(&budget);
    let completed = Mutex::new(false);
    let subscription = agent.subscribe(Arc::new(move |v, _| {
        if matches!(&*v.read().unwrap(), AgentEvent::TurnEnd { .. }) {
            let first = {
                let mut completed = completed.lock().unwrap();
                let first = !*completed;
                *completed = true;
                first
            };
            if first {
                edited.upgrade().unwrap().write().unwrap().high = Some(30.0);
            }
        }
        ready(Ok(()))
    }));
    agent.follow_up(super::user("follow"));
    let observe = agent.prompt(owned::input("prompt"));
    let joined = tokio::spawn(tasks.take());
    entry.await.unwrap();
    replace_retained(&agent, &model, &budget);
    drop(budget);
    assert!(weak.upgrade().is_some());
    release.send(()).unwrap();
    joined.await.unwrap();
    observe.await.unwrap();
    assert!(weak.upgrade().is_none());
    subscription.unsubscribe();
    let observe = agent.prompt(owned::input("next"));
    tasks.take().await;
    observe.await.unwrap();
    compare(
        "retained_budget_and_model",
        None,
        json!({"records":*records.lock().unwrap(),"providers":*providers.lock().unwrap()}),
    );
}

/// Replace live slots while independently editing the admitted object handles.
fn replace_retained(
    agent: &Agent,
    model: &Arc<std::sync::RwLock<maestro_models::Model>>,
    budget: &Arc<std::sync::RwLock<maestro_models::ThinkingBudgets>>,
) {
    budget.write().unwrap().high = Some(20.0);
    *agent.thinking_budgets.write().unwrap() = Some(Arc::new(std::sync::RwLock::new(
        maestro_models::ThinkingBudgets {
            high: Some(99.0),
            ..Default::default()
        },
    )));
    *agent.session_id.write().unwrap() = Some("new-session".into());
    let mut replacement = super::model();
    replacement.id = "replacement".into();
    replacement.provider = "replacement".into();
    agent.state().write().unwrap().model = Arc::new(std::sync::RwLock::new(replacement));
    model.write().unwrap().provider = "after".into();
    model.write().unwrap().id = "edited".into();
}

/// Observe queue and cancellation state immediately after the selected control.
fn apply_control(agent: &Agent, action: &str) -> Value {
    agent.steer(super::user("pending"));
    agent.follow_up(super::user("follow"));
    let signal = agent.signal().unwrap();
    match action {
        "abort" => agent.abort(),
        "reset" => agent.reset(),
        _ => panic!("unknown control action"),
    }
    json!({"queued":agent.has_queued_messages(),"aborted":signal.is_aborted(),"signalPresent":agent.signal().is_some()})
}

/// A busy continuation does not consume input even after history reset.
async fn busy_continuation(agent: &Arc<Agent>, reset: bool) -> (String, bool) {
    if reset {
        agent.reset();
    }
    agent.steer(super::user("zulu"));
    let error = agent.r#continue().await.unwrap_err().message;
    let queued = agent.has_queued_messages();
    (error, queued)
}
