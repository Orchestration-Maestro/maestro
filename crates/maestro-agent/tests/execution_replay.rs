//! Recorded observations through both public execution representations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::*;
use maestro_models::{Cancellation, DiagnosticErrorInfo, SharedAssistantMessage};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

/// A recorded input with its entire output and exact generated-time omissions.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    /// Scenario identity used for failure attribution.
    id: String,
    /// Operands consumed by the scenario.
    input: BTreeMap<String, Value>,
    /// Independently recorded observations.
    expected: Value,
    /// Exact paths of generated tool-result times, relative to the output.
    omissions: Vec<String>,
}
/// Snapshot a standard message without dropping optional fields.
fn message(value: &AgentMessage) -> Value {
    serde_json::to_value(value.to_message().unwrap()).unwrap()
}
/// Compare JSON values numerically, retaining all object members and array order.
fn compare(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(a), Value::Number(b)) => assert_eq!(a.as_f64(), b.as_f64(), "{path}"),
        (Value::Array(a), Value::Array(b)) => {
            assert_eq!(a.len(), b.len(), "{path}: array length");
            for (i, (a, b)) in a.iter().zip(b).enumerate() {
                compare(a, b, &format!("{path}/{i}"));
            }
        }
        (Value::Object(a), Value::Object(b)) => {
            let mut a_keys = a.keys().collect::<Vec<_>>();
            let mut b_keys = b.keys().collect::<Vec<_>>();
            a_keys.sort();
            b_keys.sort();
            assert_eq!(a_keys, b_keys, "{path}: members");
            for (key, a) in a {
                compare(a, &b[key], &format!("{path}/{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}
/// Remove only the individually enumerated generated timestamps.
fn check(case: &Case, mut actual: Value) {
    for path in &case.omissions {
        let (parent, key) = path.rsplit_once('/').unwrap();
        assert_eq!(key, "timestamp");
        let object = actual.pointer_mut(parent).unwrap().as_object_mut().unwrap();
        assert_eq!(object["role"], "toolResult");
        assert!(object.shift_remove(key).unwrap().is_number());
    }
    compare(&actual, &case.expected, &case.id);
}
/// Produce the exact authored error at a controlled callback boundary.
fn failure(text: &str) -> DiagnosticErrorInfo {
    support::owned::failure(text)
}
/// Match the terminating tool response in the recording.
fn result() -> AgentToolResult {
    serde_json::from_value(json!({"content":[{"type":"text","text":"result"}],"details":{"base":true},"terminate":true})).unwrap()
}
/// Construct the source's authoritative model response.
fn response(behavior: &str, first: bool) -> SharedAssistantMessage {
    let response = support::assistant("done");
    if first {
        let value = if behavior == "prepared" {
            json!("7")
        } else if behavior == "invalid" {
            json!("bad")
        } else {
            json!(1)
        };
        let name = if behavior == "unknown" {
            "absent"
        } else {
            "echo"
        };
        let mut response = response.write().unwrap();
        response.content = serde_json::from_value(json!([
            {"type":"toolCall","id":"first","name":name,"arguments":{"value":value}},
            {"type":"toolCall","id":"second","name":"echo","arguments":{"value":2}}
        ]))
        .unwrap();
        response.stop_reason = maestro_models::StopReason::ToolUse;
    }
    response
}
/// Compare one ownership recording in full, rejecting unread case members.
fn ownership(id: &str, actual: &Value) {
    let cases: Vec<Value> =
        serde_json::from_str(include_str!("fixtures/execution-ownership.json")).unwrap();
    let mut expected = cases.into_iter().find(|row| row["case"] == id).unwrap();
    expected.as_object_mut().unwrap().shift_remove("case");
    compare(actual, &expected, id);
}
/// Run identical callback construction with the concrete public pointer families.
macro_rules! replay {
    ($namespace:ident, $mode:ty, $pointer:ident, $new:ident, $run:path, $options:expr) => {
        pub(crate) mod $namespace {
            use super::*;
            type Mode = $mode;
            type Owner<T> = $pointer<T>;
            type Trace = Owner<Mutex<Vec<Value>>>;
            type Signals = Owner<Mutex<Vec<Cancellation>>>;
            type Task = <Mode as Execution>::Future<'static, ()>;
            /// Replay the retained local FIFO item and its independent result.
            pub(crate) fn fifo() {
                let cases: Vec<Case> =
                    serde_json::from_str(include_str!("fixtures/execution-affinity.json")).unwrap();
                let case = cases
                    .iter()
                    .find(|case| case.id == "fifo-retained-item")
                    .unwrap();
                let mut input = case.input.clone();
                assert_eq!(input.remove("pushBeforeEnd").unwrap(), true);
                let end = input.remove("endResult").unwrap().as_i64().unwrap();
                assert_eq!(input.remove("pushAfterEnd").unwrap(), true);
                assert!(input.is_empty());
                let held = Owner::new("retained");
                let weak = Owner::downgrade(&held);
                let queue = maestro_models::EventStream::new_local_items(|_: &Owner<&str>| false, |_| 0);
                queue.push(held);
                queue.end(Some(end));
                let final_result = support::run(queue.result());
                let mut items = vec![];
                while let Some(item) = support::run(queue.next()) {
                    items
                        .push(json!({"value":*item,"sameIdentity":Owner::ptr_eq(&weak.upgrade().unwrap(),&item)}));
                }
                assert!(weak.upgrade().is_none());
                queue.push(Owner::new("late"));
                assert!(support::run(queue.next()).is_none());
                check(case, json!({"final":final_result,"items":items}));
            }
            /// Snapshot every retained message field in source order.
            fn history(agent: &Agent<NoCustomMessages, Mode>) -> Vec<Value> {
                agent
                    .state()
                    .read()
                    .unwrap()
                    .messages()
                    .read()
                    .unwrap()
                    .iter()
                    .map(message)
                    .collect()
            }
            /// A suspended listener or hook's one-shot gate.
            type Waiter = Owner<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>;
            /// A tool-completion gate released by a listener.
            type Release = Owner<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>;
            /// Retain the observed handle for a post-run cancellation-link witness.
            fn signal(signals: &Signals, value: Option<Cancellation>) -> usize {
                let mut signals = signals.lock().unwrap();
                let index = signals.len();
                signals.push(value.unwrap());
                index
            }
            /// Resolve handle linkage after aborting the still-live run at its last event.
            fn resolve_signals(rows: &mut [Value], signals: &[Cancellation]) {
                for row in rows {
                    if let Some(index) = row.get("sameSignal").and_then(Value::as_u64) {
                        row["sameSignal"] = json!(signals[usize::try_from(index).unwrap()].is_aborted());
                    }
                }
            }
            /// Build a real tool with distinguishable argument/result/error branches.
            fn tool(behavior: &str, trace: Trace, signals: Signals) -> AgentToolRecord<Mode> {
                let behavior = behavior.to_owned();
                let execute_behavior = behavior.clone();
                let executed = trace.clone();
                let mut tool = AgentToolRecord::<Mode>::typed::<Value, Value>(
                    maestro_models::Tool {
                        name: "echo".into(),
                        description: "Echo".into(),
                        parameters: json!({"type":"object","properties":{"value":{"type":"number"}},"required":["value"]}).into(),
                    },
                    "Echo".into(),
                    Owner::new(move |id, args: Value, seen_signal, update| {
                        executed.lock().unwrap().push(json!({"call":"execute","id":id,"args":args,"sameSignal":signal(&signals,seen_signal)}));
                        let outcome = if execute_behavior == "execute-error" {
                            Err(failure("execution failed"))
                        } else {
                            if execute_behavior == "progress" {
                                update.unwrap()(
                                    serde_json::from_value(
                                        json!({"content":[],"details":{"partial":true}}),
                                    )
                                    .unwrap(),
                                );
                                let mut trace = executed.lock().unwrap();
                                let prefix = trace.iter().any(|row| row["call"] == "progress-prefix");
                                trace.push(json!({"call":"after-update","prefixSeen":prefix}));
                            }
                            let mut value = result();
                            if execute_behavior == "queued-input" {
                                value.terminate = Some(false);
                            }
                            Ok(value)
                        };
                        Box::pin(std::future::ready(outcome))
                    }),
                );
                if behavior == "prepared" {
                    tool.prepare_arguments = Some(Owner::new(move |raw| {
                        trace
                            .lock()
                            .unwrap()
                            .push(json!({"call":"prepare","args":raw}));
                        let value = raw["value"].as_str().map_or_else(
                            || raw["value"].as_f64().unwrap(),
                            |v| v.parse::<f64>().unwrap(),
                        );
                        Ok(json!({"value":value}))
                    }));
                }
                tool
            }
            /// Construct the callbacks without capturing their eventual agent owner.
            fn options(
                behavior: &str,
                trace: Trace,
                requests: Trace,
                signals: Signals,
            ) -> AgentOptions<NoCustomMessages, Mode> {
                let before_trace = trace.clone();
                let before_signals = signals.clone();
                let before_behavior = behavior.to_owned();
                let after_trace = trace.clone();
                let after_signals = signals.clone();
                let after_behavior = behavior.to_owned();
                let stream_behavior = behavior.to_owned();
                let count = Owner::new(Mutex::new(0));
                AgentOptions {
                    initial_state: AgentInitialState {
                        model: Some(Arc::new(RwLock::new(support::model()))),
                        tools: vec![Owner::new(RwLock::new(tool(
                            behavior,
                            trace,
                            signals.clone(),
                        )))],
                        ..Default::default()
                    },
                    before_tool_call: Some(Owner::new(move |context, seen_signal| {
                        before_trace.lock().unwrap().push(json!({"call":"before","id":context.tool_call.id,"args":context.args,"sameSignal":signal(&before_signals,seen_signal)}));
                        context.args["checked"] = json!(true);
                        let value = match before_behavior.as_str() {
                            "before-block" => Ok(Some(BeforeToolCallResult {
                                block: true,
                                reason: Some("blocked here".into()),
                            })),
                            "before-error" => Err(failure("before failed")),
                            _ => Ok(None),
                        };
                        Box::pin(std::future::ready(value))
                    })),
                    after_tool_call: Some(Owner::new(move |context, seen_signal| {
                        after_trace.lock().unwrap().push(json!({"call":"after","id":context.tool_call.id,"args":context.args,"rawArgs":context.tool_call.arguments,"result":support::observations::tool_result(context.result),"isError":context.is_error,"sameSignal":signal(&after_signals,seen_signal)}));
                        let value = match after_behavior.as_str() {
                            "after-error" => Err(failure("after failed")),
                            "overrides" => Ok(Some(AfterToolCallResult {
                                content: Some(vec![]),
                                details: Some(Value::Null),
                                is_error: Some(false),
                                terminate: Some(true),
                            })),
                            _ => Ok(None),
                        };
                        Box::pin(std::future::ready(value))
                    })),
                    stream_fn: Some(Owner::new(move |_, context, options, _| {
                        requests.lock().unwrap().push(json!({"messages":context.messages,"reasoning":options.reasoning,"sameSignal":signal(&signals,options.common.signal)}));
                        let mut count = count.lock().unwrap();
                        *count += 1;
                        let stream = maestro_models::AssistantMessageEventStream::new();
                        stream.end(Some(response(&stream_behavior, *count == 1)));
                        Box::pin(std::future::ready(Ok(stream)))
                    })),
                    ..Default::default()
                }
            }
            /// Replay one public agent transaction and retain the complete transcript.
            fn ordinary(case: &Case) {
                let mut input = case.input.clone();
                let behavior = input
                    .remove("behavior")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned();
                let mode = input.remove("mode").unwrap();
                assert!(input.is_empty());
                let trace = Owner::new(Mutex::new(vec![]));
                let events = Owner::new(Mutex::new(vec![]));
                let requests = Owner::new(Mutex::new(vec![]));
                let signals = Owner::new(Mutex::new(vec![]));
                let tasks = Owner::new(Mutex::new(Vec::<Task>::new()));
                let mut options = options(&behavior, trace.clone(), requests.clone(), signals.clone());
                options.tool_execution = Some(serde_json::from_value(mode).unwrap());
                let scheduled = tasks.clone();
                options.spawn = Some(Owner::new(move |task| scheduled.lock().unwrap().push(task)));
                let agent = Owner::new(Agent::<NoCustomMessages, Mode>::$new(options));
                let observed = events.clone();
                let progress = trace.clone();
                let weak = Owner::downgrade(&agent);
                agent.subscribe(Owner::new(move |event, _| {
                    let event = event.read().unwrap();
                    observed
                        .lock()
                        .unwrap()
                        .push(support::observations::event(&event, message));
                    if let AgentEvent::ToolExecutionUpdate { tool_call, .. } = &*event {
                        progress
                            .lock()
                            .unwrap()
                            .push(json!({"call":"progress-prefix","id":tool_call.id}));
                    }
                    if matches!(*event, AgentEvent::AgentEnd { .. }) {
                        weak.upgrade().unwrap().abort();
                    }
                    Box::pin(std::future::ready(Ok(())))
                }));
                if behavior == "queued-input" {
                    let follow = support::user("follow-up");
                    if let AgentMessage::User(value) = &follow {
                        value.write().unwrap().timestamp = 3.0;
                    }
                    agent.follow_up(follow);
                }
                let observe = agent.prompt(PromptInput::Message(support::user("run")));
                let task = tasks.lock().unwrap().pop().unwrap();
                support::run(task);
                let outcome = support::run(observe);
                let mut trace = trace.lock().unwrap().clone();
                let mut requests = requests.lock().unwrap().clone();
                resolve_signals(&mut trace, &signals.lock().unwrap());
                resolve_signals(&mut requests, &signals.lock().unwrap());
                let history = history(&agent);
                let mut actual = json!({"trace":trace,"events":*events.lock().unwrap(),"requests":requests,"history":history,"isStreaming":agent.state().read().unwrap().is_streaming(),"signalAbsent":agent.signal().is_none()});
                if let Err(error) = outcome {
                    actual["error"] = json!(error.message);
                }
                check(case, actual);
            }
            /// Construct the two tools whose completion order is externally gated.
            fn suspended_tool(
                trace: &Trace,
                first_wait: Owner<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
            ) -> AgentToolRecord<Mode> {
                let executed = trace.clone();
                AgentToolRecord::<Mode>::typed::<Value, Value>(
                    maestro_models::Tool {
                        name: "echo".into(),
                        description: "Echo".into(),
                        parameters: json!({"type":"object","properties":{"value":{"type":"number"}},"required":["value"]}).into(),
                    },
                    "Echo".into(),
                    Owner::new(move |id, args, _, update| {
                        executed
                            .lock()
                            .unwrap()
                            .push(json!({"call":"execute","id":id,"args":args}));
                        let wait = if id == "first" {
                            first_wait.lock().unwrap().take()
                        } else {
                            update.unwrap()(
                                serde_json::from_value(json!({"content":[],"details":{"partial":true}}))
                                    .unwrap(),
                            );
                            let mut trace = executed.lock().unwrap();
                            let prefix = trace.iter().any(|row| row["call"] == "progress-prefix");
                            trace.push(json!({"call":"after-update","prefixSeen":prefix}));
                            None
                        };
                        Box::pin(async move {
                            if let Some(wait) = wait {
                                wait.await.unwrap();
                            }
                            Ok(result())
                        })
                    }),
                )
            }
            /// Build callbacks whose borrowed arguments remain live across the hook gate.
            fn suspended_agent(
                tasks: &Owner<Mutex<Vec<Task>>>,
                trace: &Trace,
                tool: AgentToolRecord<Mode>,
                hook_wait: Waiter,
            ) -> Owner<Agent<NoCustomMessages, Mode>> {
                let scheduled = tasks.clone();
                let before = trace.clone();
                let after = trace.clone();
                Owner::new(Agent::<NoCustomMessages, Mode>::$new(
                    AgentOptions {
                        initial_state: AgentInitialState {
                            model: Some(Arc::new(RwLock::new(support::model()))),
                            tools: vec![Owner::new(RwLock::new(tool))],
                            ..Default::default()
                        },
                        spawn: Some(Owner::new(move |task| scheduled.lock().unwrap().push(task))),
                        stream_fn: Some(Owner::new(|_, _, _, _| {
                            let stream = maestro_models::AssistantMessageEventStream::new();
                            stream.end(Some(response("ordinary", true)));
                            Box::pin(std::future::ready(Ok(stream)))
                        })),
                        before_tool_call: Some(Owner::new(move |context, _| {
                            before
                                .lock()
                                .unwrap()
                                .push(json!({"call":"before-enter","id":context.tool_call.id}));
                            let wait = hook_wait.lock().unwrap().take();
                            let before = before.clone();
                            Box::pin(async move {
                                if let Some(wait) = wait {
                                    wait.await.unwrap();
                                }
                                context.args["checked"] = json!(true);
                                before
                                    .lock()
                                    .unwrap()
                                    .push(json!({"call":"before-exit","id":context.tool_call.id}));
                                Ok(None)
                            })
                        })),
                        after_tool_call: Some(Owner::new(move |context, _| {
                            after
                                .lock()
                                .unwrap()
                                .push(json!({"call":"after","id":context.tool_call.id}));
                            Box::pin(std::future::ready(Ok(None)))
                        })),
                        ..Default::default()
                    },
                ))
            }
            /// Observe events eagerly, delaying settlement at progress and final-event gates.
            fn suspended_listener(
                events: &Trace,
                trace: &Trace,
                gates: (Waiter, Waiter, Release),
            ) -> AgentListener<NoCustomMessages, Mode> {
                let (progress_wait, end_wait, first_release) = gates;
                let observed = events.clone();
                let seen = trace.clone();
                Owner::new(move |event, _| {
                    let event = event.read().unwrap();
                    observed
                        .lock()
                        .unwrap()
                        .push(support::observations::event(&event, message));
                    let wait = match &*event {
                        AgentEvent::ToolExecutionUpdate { tool_call, .. } => {
                            seen.lock()
                                .unwrap()
                                .push(json!({"call":"progress-prefix","id":tool_call.id}));
                            progress_wait.lock().unwrap().take()
                        }
                        AgentEvent::ToolExecutionEnd { tool_call, .. } => {
                            seen.lock()
                                .unwrap()
                                .push(json!({"call":"end","id":tool_call.id}));
                            if tool_call.id == "second" {
                                let release = first_release.lock().unwrap().take().unwrap();
                                release.send(()).unwrap();
                            }
                            None
                        }
                        AgentEvent::AgentEnd { .. } => end_wait.lock().unwrap().take(),
                        _ => None,
                    };
                    Box::pin(async move {
                        if let Some(wait) = wait {
                            wait.await.unwrap();
                        }
                        Ok(())
                    })
                })
            }
            /// Drive genuine suspension and compare the complete recorded owner transaction.
            pub(crate) fn suspended_owner() {
                use futures_util::FutureExt;
                let cases: Vec<Case> =
                    serde_json::from_str(include_str!("fixtures/execution-affinity.json")).unwrap();
                let case = cases
                    .iter()
                    .find(|case| case.id == "suspended-parallel-owner")
                    .unwrap();
                let mut input = case.input.clone();
                assert_eq!(input.remove("mode").unwrap(), "parallel");
                assert_eq!(
                    input.remove("gates").unwrap(),
                    json!(["hook", "progress", "end"])
                );
                assert_eq!(input.remove("sameListenerTwice").unwrap(), true);
                assert!(input.is_empty());
                let trace: Trace = Owner::new(Mutex::new(vec![]));
                let events: Trace = Owner::new(Mutex::new(vec![]));
                let tasks = Owner::new(Mutex::new(Vec::<Task>::new()));
                let (hook_release, hook_wait) = tokio::sync::oneshot::channel();
                let hook_wait = Owner::new(Mutex::new(Some(hook_wait)));
                let (progress_release, progress_wait) = tokio::sync::oneshot::channel();
                let progress_wait = Owner::new(Mutex::new(Some(progress_wait)));
                let (end_release, end_wait) = tokio::sync::oneshot::channel();
                let end_wait = Owner::new(Mutex::new(Some(end_wait)));
                let (first_release, first_wait) = tokio::sync::oneshot::channel();
                let first_wait = Owner::new(Mutex::new(Some(first_wait)));
                let first_release = Owner::new(Mutex::new(Some(first_release)));
                let tool = suspended_tool(&trace, first_wait);
                let agent = suspended_agent(&tasks, &trace, tool, hook_wait);
                let listener =
                    suspended_listener(&events, &trace, (progress_wait, end_wait, first_release));
                let remove = agent.subscribe(listener.clone());
                agent.subscribe(listener);
                let observe = agent.prompt(PromptInput::Message(support::user("run")));
                drop(observe);
                let mut task = tasks.lock().unwrap().pop().unwrap();
                assert!(task.as_mut().now_or_never().is_none());
                let before_release = trace.lock().unwrap().clone();
                hook_release.send(()).unwrap();
                assert!(task.as_mut().now_or_never().is_none());
                let progress_pending = trace.lock().unwrap().clone();
                progress_release.send(()).unwrap();
                assert!(task.as_mut().now_or_never().is_none());
                let before_end = json!({"streaming":agent.state().read().unwrap().is_streaming(),"signalPresent":agent.signal().is_some()});
                let idle = agent.wait_for_idle();
                assert!(agent.wait_for_idle().now_or_never().is_none());
                end_release.send(()).unwrap();
                support::run(task);
                support::run(idle);
                remove.unsubscribe();
                remove.unsubscribe();
                suspended_output(
                    case,
                    &agent,
                    &trace,
                    &events,
                    (before_release, progress_pending, before_end),
                );
                drop(agent);
                assert_eq!(Owner::strong_count(&trace), 1);
            }
            /// Check complete output after every admitted future has settled.
            fn suspended_output(
                case: &Case,
                agent: &Agent<NoCustomMessages, Mode>,
                trace: &Trace,
                events: &Trace,
                phases: (Vec<Value>, Vec<Value>, Value),
            ) {
                let (before_release, progress_pending, before_end) = phases;
                let trace = trace.lock().unwrap().clone();
                let history = history(&agent);
                let completion = trace
                    .iter()
                    .filter(|row| row["call"] == "end")
                    .map(|row| row["id"].clone())
                    .collect::<Vec<_>>();
                let artifacts = history
                    .iter()
                    .filter(|row| row["role"] == "toolResult")
                    .map(|row| row["toolCallId"].clone())
                    .collect::<Vec<_>>();
                check(
                    case,
                    json!({"beforeRelease":before_release,"progressPending":progress_pending,"beforeEnd":before_end,"trace":trace,"events":*events.lock().unwrap(),"completionOrder":completion,"artifactOrder":artifacts,"history":history,"isStreaming":agent.state().read().unwrap().is_streaming(),"signalAbsent":agent.signal().is_none()}),
                );
            }
            #[test]
            fn live_descriptions_retain_the_selected_callable_owner() {
                let (agent, tasks) = simple_agent();
                let description = Owner::new(Mutex::new(String::from("first")));
                let captured = description.clone();
                let mut tool = stop_tool();
                tool.description = Some(Owner::new(move || Ok(captured.lock().unwrap().clone())));
                agent
                    .state()
                    .write()
                    .unwrap()
                    .set_tools(vec![Owner::new(RwLock::new(tool))]);
                let observed = Owner::new(Mutex::new(vec![]));
                let seen = observed.clone();
                *agent.stream_fn.write().unwrap() = Some(Owner::new(move |_, context, _, _| {
                    seen.lock()
                        .unwrap()
                        .push(context.tools.unwrap()[0].description.clone());
                    let stream = maestro_models::AssistantMessageEventStream::new();
                    stream.end(Some(response("ordinary", false)));
                    Box::pin(std::future::ready(Ok(stream)))
                }));
                complete(
                    &tasks,
                    agent.prompt(PromptInput::Message(support::user("one"))),
                );
                *description.lock().unwrap() = "second".into();
                complete(
                    &tasks,
                    agent.prompt(PromptInput::Message(support::user("two"))),
                );
                assert_eq!(*observed.lock().unwrap(), ["first", "second"]);
                drop(agent);
                assert_eq!(Owner::strong_count(&description), 1);
            }
            /// Construct a test-owned task queue with a terminating offline response.
            fn simple_agent() -> (Owner<Agent<NoCustomMessages, Mode>>, Owner<Mutex<Vec<Task>>>) {
                let tasks = Owner::new(Mutex::new(Vec::<Task>::new()));
                let scheduled = tasks.clone();
                let agent = Owner::new(Agent::<NoCustomMessages, Mode>::$new(
                    AgentOptions {
                        spawn: Some(Owner::new(move |task| scheduled.lock().unwrap().push(task))),
                        stream_fn: Some(Owner::new(|_, _, _, _| {
                            let stream = maestro_models::AssistantMessageEventStream::new();
                            stream.end(Some(response("ordinary", false)));
                            Box::pin(std::future::ready(Ok(stream)))
                        })),
                        ..Default::default()
                    },
                ));
                (agent, tasks)
            }
            /// Drive a retained task without holding its queue guard.
            fn complete(
                tasks: &Owner<Mutex<Vec<Task>>>,
                observe: maestro_models::BoxFuture<Result<(), DiagnosticErrorInfo>>,
            ) {
                let task = tasks.lock().unwrap().pop().unwrap();
                support::run(task);
                support::run(observe).unwrap();
            }
            #[test]
            fn dropped_observer_preserves_admitted_work() {
                use futures_util::FutureExt;
                let (agent, tasks) = simple_agent();
                let effect = Owner::new(Mutex::new(false));
                let effected = effect.clone();
                let (release, wait) = tokio::sync::oneshot::channel();
                let wait = Owner::new(Mutex::new(Some(wait)));
                agent.subscribe(Owner::new(move |event, _| {
                    let wait = if matches!(*event.read().unwrap(), AgentEvent::AgentEnd { .. }) {
                        wait.lock().unwrap().take()
                    } else {
                        None
                    };
                    let effect = effected.clone();
                    Box::pin(async move {
                        if let Some(wait) = wait {
                            wait.await.unwrap();
                            *effect.lock().unwrap() = true;
                        }
                        Ok(())
                    })
                }));
                drop(agent.prompt(PromptInput::Message(support::user("drop observation"))));
                let mut task = tasks.lock().unwrap().pop().unwrap();
                assert!(task.as_mut().now_or_never().is_none());
                let busy = support::run(agent.prompt(PromptInput::Message(support::user("second"))))
                    .unwrap_err()
                    .message;
                let before = json!({"streaming":agent.state().read().unwrap().is_streaming(),"effect":*effect.lock().unwrap()});
                let idle = agent.wait_for_idle();
                agent.abort();
                let aborted = agent.signal().unwrap().is_aborted();
                release.send(()).unwrap();
                support::run(task);
                support::run(idle);
                ownership(
                    "dropped-observer",
                    &json!({"before":before,"busy":busy,"aborted":aborted,"after":{"streaming":agent.state().read().unwrap().is_streaming(),"effect":*effect.lock().unwrap(),"signalAbsent":agent.signal().is_none()}}),
                );
            }
            /// Install live insertion/removal with weak captures of the enclosing owner.
            fn live_listeners(
                agent: &Owner<Agent<NoCustomMessages, Mode>>,
                order: &Owner<Mutex<Vec<String>>>,
            ) -> (
                Owner<Mutex<Option<AgentListener<NoCustomMessages, Mode>>>>,
                Owner<Mutex<Option<Subscription<NoCustomMessages, Mode>>>>,
            ) {
                let first = Owner::new(Mutex::new(true));
                let remove_a = Owner::new(Mutex::new(None::<Subscription<NoCustomMessages, Mode>>));
                let remove_b = Owner::new(Mutex::new(None::<Subscription<NoCustomMessages, Mode>>));
                let a_slot = Owner::new(Mutex::new(None::<AgentListener<NoCustomMessages, Mode>>));
                let seen = order.clone();
                let c: AgentListener<NoCustomMessages, Mode> = Owner::new(move |event, _| {
                    if matches!(*event.read().unwrap(), AgentEvent::AgentStart) {
                        seen.lock().unwrap().push("c".into());
                    }
                    Box::pin(std::future::ready(Ok(())))
                });
                let seen = order.clone();
                let weak = Owner::downgrade(&agent);
                let a_remove = remove_a.clone();
                let b_remove = remove_b.clone();
                let a_weak = Owner::downgrade(&a_slot);
                let a: AgentListener<NoCustomMessages, Mode> = Owner::new(move |event, _| {
                    if matches!(*event.read().unwrap(), AgentEvent::AgentStart) {
                        seen.lock().unwrap().push("a".into());
                        let initial = std::mem::replace(&mut *first.lock().unwrap(), false);
                        if initial {
                            b_remove.lock().unwrap().as_ref().unwrap().unsubscribe();
                            let agent = weak.upgrade().unwrap();
                            agent.subscribe(c.clone());
                            agent.subscribe(c.clone());
                            a_remove.lock().unwrap().as_ref().unwrap().unsubscribe();
                            let a = a_weak.upgrade().unwrap().lock().unwrap().clone().unwrap();
                            agent.subscribe(a);
                        }
                    }
                    Box::pin(std::future::ready(Ok(())))
                });
                *a_slot.lock().unwrap() = Some(a.clone());
                *remove_a.lock().unwrap() = Some(agent.subscribe(a.clone()));
                agent.subscribe(a);
                let seen = order.clone();
                *remove_b.lock().unwrap() = Some(agent.subscribe(Owner::new(move |event, _| {
                    if matches!(*event.read().unwrap(), AgentEvent::AgentStart) {
                        seen.lock().unwrap().push("b".into());
                    }
                    Box::pin(std::future::ready(Ok(())))
                })));
                (a_slot, remove_a)
            }
            #[test]
            fn listeners_keep_live_identity_and_old_removers() {
                let (agent, tasks) = simple_agent();
                let order = Owner::new(Mutex::new(Vec::<String>::new()));
                let (a_slot, remove_a) = live_listeners(&agent, &order);
                complete(
                    &tasks,
                    agent.prompt(PromptInput::Message(support::user("live"))),
                );
                let first_order = order.lock().unwrap().clone();
                remove_a.lock().unwrap().as_ref().unwrap().unsubscribe();
                remove_a.lock().unwrap().as_ref().unwrap().unsubscribe();
                order.lock().unwrap().clear();
                complete(
                    &tasks,
                    agent.prompt(PromptInput::Message(support::user("next"))),
                );
                ownership(
                    "live-listeners",
                    &json!({"firstOrder":first_order,"nextOrder":*order.lock().unwrap()}),
                );
                drop(agent);
                drop(a_slot);
                assert_eq!(Owner::strong_count(&order), 1);
            }
            #[test]
            fn listeners_share_event_edits_not_reduced_state() {
                let (agent, tasks) = simple_agent();
                let observed = Owner::new(Mutex::new(Value::Null));
                agent.subscribe(Owner::new(|event, _| {
                    let mut event = event.write().unwrap();
                    if let AgentEvent::MessageStart { message } = &mut *event {
                        if matches!(message, AgentMessage::User(_)) {
                            *message = support::user("changed event");
                        }
                    }
                    Box::pin(std::future::ready(Ok(())))
                }));
                let seen = observed.clone();
                let weak = Owner::downgrade(&agent);
                agent.subscribe(Owner::new(move |event, _| {
                    if let AgentEvent::MessageStart { message: entry } = &*event.read().unwrap() {
                        if matches!(entry, AgentMessage::User(_)) {
                            let agent = weak.upgrade().unwrap();
                            let state = agent.state().read().unwrap();
                            let original = message(state.streaming_message().unwrap());
                            *seen.lock().unwrap() =
                                json!({"event":message(entry)["content"],"state":original["content"]});
                        }
                    }
                    Box::pin(std::future::ready(Ok(())))
                }));
                complete(
                    &tasks,
                    agent.prompt(PromptInput::Text {
                        text: "original".into(),
                        images: vec![],
                    }),
                );
                ownership(
                    "mutable-event",
                    &json!({"observation":*observed.lock().unwrap()}),
                );
            }
            #[test]
            fn admission_captures_handles_and_reset_keeps_the_run() {
                use futures_util::FutureExt;
                let (agent, tasks) = simple_agent();
                let old = Arc::new(RwLock::new(support::model()));
                old.write().unwrap().id = "old".into();
                agent.state().write().unwrap().model = old.clone();
                agent.state().write().unwrap().system_prompt = "captured".into();
                let seen = Owner::new(Mutex::new(Value::Null));
                let observed = seen.clone();
                *agent.stream_fn.write().unwrap() = Some(Owner::new(move |model, context, _, _| {
                    *observed.lock().unwrap() = json!({"id":model.id,"prompt":context.system_prompt});
                    let stream = maestro_models::AssistantMessageEventStream::new();
                    stream.end(Some(response("ordinary", false)));
                    Box::pin(std::future::ready(Ok(stream)))
                }));
                let (release, wait) = tokio::sync::oneshot::channel();
                let wait = Owner::new(Mutex::new(Some(wait)));
                agent.subscribe(Owner::new(move |event, _| {
                    let wait = if matches!(*event.read().unwrap(), AgentEvent::AgentStart) {
                        wait.lock().unwrap().take()
                    } else {
                        None
                    };
                    Box::pin(async move {
                        if let Some(wait) = wait {
                            wait.await.unwrap();
                        }
                        Ok(())
                    })
                }));
                let history = agent.state().read().unwrap().messages().clone();
                let observe = agent.prompt(PromptInput::Message(support::user("hello")));
                let mut task = tasks.lock().unwrap().pop().unwrap();
                assert!(task.as_mut().now_or_never().is_none());
                agent.state().write().unwrap().model = Arc::new(RwLock::new(support::model()));
                old.write().unwrap().id = "mutated captured".into();
                agent.state().write().unwrap().system_prompt = "changed".into();
                agent.reset();
                let aborted = agent.signal().unwrap().is_aborted();
                release.send(()).unwrap();
                support::run(task);
                support::run(observe).unwrap();
                ownership(
                    "capture-and-reset",
                    &json!({"seen":*seen.lock().unwrap(),"signalAborted":aborted,"oldHistoryLength":history.read().unwrap().len(),"newHistoryLength":agent.state().read().unwrap().messages().read().unwrap().len()}),
                );
            }
            #[test]
            fn each_run_gets_fresh_cancellation() {
                let (agent, tasks) = simple_agent();
                let signals = Owner::new(Mutex::new(vec![]));
                let observed = signals.clone();
                agent.subscribe(Owner::new(move |event, signal| {
                    if matches!(*event.read().unwrap(), AgentEvent::AgentStart) {
                        observed.lock().unwrap().push(signal);
                    }
                    Box::pin(std::future::ready(Ok(())))
                }));
                complete(
                    &tasks,
                    agent.prompt(PromptInput::Message(support::user("one"))),
                );
                agent.abort();
                complete(
                    &tasks,
                    agent.prompt(PromptInput::Message(support::user("two"))),
                );
                let signals = signals.lock().unwrap();
                let aborted = signals
                    .iter()
                    .map(Cancellation::is_aborted)
                    .collect::<Vec<_>>();
                signals[0].abort();
                let distinct = !signals[1].is_aborted();
                ownership(
                    "fresh-signal",
                    &json!({"distinct":distinct,"aborted":aborted,"idleSignalAbsent":agent.signal().is_none()}),
                );
            }
            /// Borrow and observe each completed turn before deciding whether to stop.
            fn stop_config(
                original: &Owner<AgentContextRecord<NoCustomMessages, Mode>>,
                trace: &Trace,
                mode: Value,
                stop: bool,
            ) -> AgentLoopConfigRecord<NoCustomMessages, Mode> {
                let before = original.clone();
                let observations = trace.clone();
                AgentLoopConfigRecord {
                    model: Arc::new(RwLock::new(support::model())),
                    thinking_budgets: None,
                    options: maestro_models::SimpleStreamOptions::default(),
                    extra: maestro_models::JsonObject::new(),
                    objects: maestro_models::ProviderObjects::default(),
                    convert_to_llm: Mode::default_converter(),
                    transform_context: None,
                    get_api_key: None,
                    before_tool_call: None,
                    after_tool_call: None,
                    get_steering_messages: None,
                    get_follow_up_messages: None,
                    tool_execution: Some(serde_json::from_value(mode).unwrap()),
                    should_stop_after_turn: Some(Owner::new(move |context| {
                        observations.lock().unwrap().push(json!({"call":"stop-after","sameContext":std::ptr::eq(context.context,&*before),"newRoles":context.new_messages.iter().map(|m|message(m)["role"].clone()).collect::<Vec<_>>(),"results":context.tool_results.iter().map(|m|m.tool_call_id.clone()).collect::<Vec<_>>()}));
                        Box::pin(std::future::ready(Ok(stop)))
                    })),
                    prepare_next_turn: None,
                }
            }
            /// Return a nonterminating empty tool result for stop-decision cases.
            fn stop_tool() -> AgentToolRecord<Mode> {
                AgentToolRecord::<Mode>::typed::<Value, Value>(
                    maestro_models::Tool {
                        name: "echo".into(),
                        description: "Echo".into(),
                        parameters: json!({"type":"object","properties":{}}).into(),
                    },
                    "Echo".into(),
                    Owner::new(|_, _: Value, _, _| {
                        Box::pin(std::future::ready(Ok(serde_json::from_value(
                            json!({"content":[],"details":{},"terminate":false}),
                        )
                        .unwrap())))
                    }),
                )
            }
            /// Replay the stop decision through the raw awaited operation.
            fn stop_case(case: &Case) {
                let mut input = case.input.clone();
                let stop = input.remove("stop").unwrap().as_bool().unwrap();
                let mode = input.remove("mode").unwrap();
                assert!(input.is_empty());
                let trace = Owner::new(Mutex::new(vec![]));
                let events = Owner::new(Mutex::new(vec![]));
                let requests = Owner::new(Mutex::new(0));
                let tool = stop_tool();
                let original = Owner::new(AgentContextRecord::<NoCustomMessages, Mode> {
                    system_prompt: String::new(),
                    messages: Arc::new(RwLock::new(vec![])),
                    tools: Some(Owner::new(RwLock::new(vec![Owner::new(RwLock::new(tool))]))),
                });
                let config = stop_config(&original, &trace, mode, stop);
                let requested = requests.clone();
                let options: maestro_agent::agent_loop::AgentLoopOptionsRecord<Mode> =
                    maestro_agent::agent_loop::AgentLoopOptionsRecord {
                        signal: Some(Cancellation::new()),
                        stream_fn: Some(Owner::new(move |_, _, _, _| {
                            let mut count = requested.lock().unwrap();
                            *count += 1;
                            let response = support::assistant("done");
                            if *count == 1 {
                                let mut response = response.write().unwrap();
                                response.content = serde_json::from_value(
                                    json!([{ "type":"toolCall","id":"only","name":"echo","arguments":{} }]),
                                )
                                .unwrap();
                                response.stop_reason = maestro_models::StopReason::ToolUse;
                            }
                            let stream = maestro_models::AssistantMessageEventStream::new();
                            stream.end(Some(response));
                            Box::pin(std::future::ready(Ok(stream)))
                        })),
                    };
                let observed = events.clone();
                let history = support::run($run(
                    vec![support::user("run")],
                    (*original).clone(),
                    &config,
                    Owner::new(move |event| {
                        observed
                            .lock()
                            .unwrap()
                            .push(support::observations::event(&event, message));
                        Box::pin(std::future::ready(Ok(())))
                    }),
                    ($options)(options),
                ))
                .unwrap();
                check(
                    case,
                    json!({"trace":*trace.lock().unwrap(),"events":*events.lock().unwrap(),"history":history.iter().map(message).collect::<Vec<_>>(),"requests":*requests.lock().unwrap()}),
                );
            }
            #[test]
            fn recorded_stop_decisions_match() {
                let cases: Vec<Case> =
                    serde_json::from_str(include_str!("fixtures/execution-affinity.json")).unwrap();
                for case in cases.iter().filter(|case| case.input.contains_key("stop")) {
                    stop_case(case);
                }
            }
            #[test]
            fn recorded_tool_behavior_matches() {
                let cases: Vec<Case> =
                    serde_json::from_str(include_str!("fixtures/execution-affinity.json")).unwrap();
                for case in cases
                    .iter()
                    .filter(|case| case.input.contains_key("behavior"))
                {
                    ordinary(case);
                }
            }
        }
    };
}
replay!(
    local,
    Local,
    Rc,
    new_local,
    maestro_agent::agent_loop::local::run_agent_loop,
    |options: maestro_agent::agent_loop::AgentLoopOptionsRecord<Local>| {
        maestro_agent::agent_loop::LocalAgentLoopOptions::ReplacementStream {
            signal: options.signal,
            stream_fn: options.stream_fn.unwrap(),
        }
    }
);
replay!(
    threaded,
    Threaded,
    Arc,
    new,
    maestro_agent::run_agent_loop,
    std::convert::identity
);
