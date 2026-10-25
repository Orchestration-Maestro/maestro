//! Sequential tool turns through the awaited conversation operation.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{AgentTool, AgentToolResult, run_agent_loop_continue};
use maestro_models::{AssistantContent, Tool, ToolCall};
use std::sync::{Arc, Mutex, RwLock};
use support::*;

/// Calls keep source order and repeats, with every result visible on the next turn.
#[test]
fn plain_tools_run_in_source_order() {
    run(support::corpus::replay(&["source-order-duplicates"]));
    run(plain_tools_run_in_source_order_case());
}
/// Execute the controlled public scenario.
async fn plain_tools_run_in_source_order_case() {
    let context = context();
    let calls = ordered_calls();
    let trace = Arc::new(Mutex::new(Vec::new()));
    let tools = ordered_tools(&trace);
    let mut context = context;
    context.tools = Some(Arc::new(RwLock::new(tools)));
    let history = Arc::clone(&context.messages);
    let observed_history = Arc::clone(&history);
    let emit = Arc::new(move |event| {
        if matches!(
            event,
            maestro_agent::AgentEvent::ToolExecutionStart { .. }
                | maestro_agent::AgentEvent::MessageStart {
                    message: maestro_agent::AgentMessage::ToolResult(_)
                }
                | maestro_agent::AgentEvent::MessageEnd {
                    message: maestro_agent::AgentMessage::ToolResult(_)
                }
        ) {
            assert_eq!(
                observed_history.read().unwrap().len(),
                2,
                "artifacts enter history only after the batch"
            );
        }
        ready(Ok(()))
    });
    let returned = run_agent_loop_continue(
        context,
        &config(),
        emit,
        responses(vec![calls, assistant("final")]),
    )
    .await
    .unwrap();
    assert_eq!(
        *trace.lock().unwrap(),
        [
            ("zulu", "0".into()),
            ("alpha", "1".into()),
            ("zulu", "2".into())
        ]
    );
    assert_eq!(returned.len(), 5);
    assert_eq!(history.read().unwrap().len(), 6);
}

/// Preparation transforms the candidate before the existing validator coerces it.
#[test]
fn preparation_precedes_validation_and_preserves_raw_events() {
    run(support::corpus::replay(&[
        "prepare-identity",
        "prepare-replace",
        "prepare-error",
    ]));
    run(preparation_precedes_validation_and_preserves_raw_events_case());
}
/// Execute the controlled public scenario.
async fn preparation_precedes_validation_and_preserves_raw_events_case() {
    let calls = assistant("call");
    calls.write().unwrap().content = vec![AssistantContent::ToolCall(ToolCall {
        id: "id".into(),
        name: "echo".into(),
        arguments: serde_json::from_value(serde_json::json!({"value":"raw"})).unwrap(),
        thought_signature: None,
    })];
    let tool = Arc::new(RwLock::new(AgentTool {
        description: None,
        definition: Tool {
            name: "echo".into(),
            description: "Echo".into(),
            parameters: serde_json::json!({"type":"object","properties":{"value":{"type":"integer"}},"required":["value"]}).into(),
        },
        label: "Echo".into(),
        execution_mode: None,
        prepare_arguments: Some(Arc::new(|value| {
            assert_eq!(value["value"], "raw");
            Ok(serde_json::json!({"value":"8"}))
        })),
        execute: Arc::new(|_, args, _, _| {
            assert_eq!(args["value"].as_f64(), Some(8.0));
            Box::pin(async {
                Ok(AgentToolResult {
                    content: vec![],
                    details: Some(serde_json::Value::Null),
                    terminate: Some(true),
                })
            })
        }),
    }));
    let mut context = context();
    context.tools = Some(Arc::new(RwLock::new(vec![tool])));
    let emit = Arc::new(|event| {
        if let maestro_agent::AgentEvent::ToolExecutionStart { tool_call }
        | maestro_agent::AgentEvent::ToolExecutionEnd { tool_call, .. } = event
        {
            assert_eq!(tool_call.arguments["value"], "raw");
        }
        Box::pin(async { Ok(()) }) as maestro_models::BoxFuture<_>
    });
    let returned = run_agent_loop_continue(context, &config(), emit, responses(vec![calls]))
        .await
        .unwrap();
    assert_eq!(returned.len(), 2);
}

/// A progress effect can release the still-running execution future.
#[test]
fn progress_runs_while_execution_is_pending() {
    run(support::corpus::replay(&["progress-1"]));
    run(progress_runs_while_execution_is_pending_case());
}
/// Execute the controlled public scenario.
async fn progress_runs_while_execution_is_pending_case() {
    use futures_util::FutureExt;
    let effect = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = Arc::clone(&effect);
    let (release, released) = tokio::sync::oneshot::channel();
    let release = Arc::new(Mutex::new(Some(release)));
    let released = Arc::new(Mutex::new(Some(released)));
    let observed = Arc::clone(&release);
    let emit = Arc::new(move |event| {
        let gate = if matches!(event, maestro_agent::AgentEvent::ToolExecutionUpdate { .. }) {
            observed.lock().unwrap().take()
        } else {
            None
        };
        let seen = Arc::clone(&seen);
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.send(()).unwrap();
                seen.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            Ok(())
        }) as maestro_models::BoxFuture<_>
    });
    let context = with_execute(Arc::new(move |_, _, _, update| {
        let gate = released.lock().unwrap().take().unwrap();
        Box::pin(async move {
            update.unwrap()(result());
            gate.await.unwrap();
            Ok(AgentToolResult {
                details: Some(serde_json::json!({"done":true})),
                ..result()
            })
        })
    }));
    let calls = tool_call();
    let config = config();
    let mut operation = Box::pin(run_agent_loop_continue(
        context,
        &config,
        emit,
        responses(vec![calls]),
    ));
    let completed = operation.as_mut().now_or_never();
    assert!(
        effect.load(std::sync::atomic::Ordering::SeqCst),
        "progress must run before execution completes"
    );
    let returned = match completed {
        Some(value) => value,
        None => operation.await,
    }
    .unwrap();
    let maestro_agent::AgentMessage::ToolResult(result) = &returned[1] else {
        panic!("expected result");
    };
    assert_eq!(
        result.read().unwrap().details,
        Some(serde_json::json!({"done":true}))
    );
}

/// Artifact callbacks and history observe the same mutable entry.
#[test]
fn tool_artifacts_keep_content_details_and_timestamps() {
    run(artifact_timestamp_case());
    run(support::corpus::replay(&[
        "details-0",
        "details-1",
        "details-2",
        "details-3",
        "details-4",
        "details-5",
        "tool-success",
    ]));
    run(tool_artifacts_keep_content_details_and_timestamps_case());
}
/// Execute the controlled public scenario.
async fn tool_artifacts_keep_content_details_and_timestamps_case() {
    let mut context = context();
    let history = Arc::clone(&context.messages);
    context.tools = Some(Arc::new(RwLock::new(vec![Arc::new(RwLock::new(
        AgentTool {
            description: None,
            definition: Tool {
                name: "echo".into(),
                description: "Echo".into(),
                parameters: serde_json::json!({"type":"object"}).into(),
            },
            label: "Echo".into(),
            prepare_arguments: None,
            execution_mode: None,
            execute: Arc::new(|_, _, _, _| {
                Box::pin(async {
                    Ok(AgentToolResult {
                        content: vec![],
                        details: Some(serde_json::json!({"original":true})),
                        terminate: Some(true),
                    })
                })
            }),
        },
    ))])));
    let retained = Arc::new(Mutex::new(None));
    let observed = Arc::clone(&retained);
    let emit = Arc::new(move |event| {
        if let maestro_agent::AgentEvent::MessageEnd {
            message: maestro_agent::AgentMessage::ToolResult(message),
        } = event
        {
            message.write().unwrap().details = Some(serde_json::json!({"edited":true}));
            *observed.lock().unwrap() = Some(message);
        }
        Box::pin(async { Ok(()) }) as maestro_models::BoxFuture<_>
    });
    let calls = assistant("calls");
    calls.write().unwrap().content = vec![AssistantContent::ToolCall(ToolCall {
        id: "id".into(),
        name: "echo".into(),
        arguments: maestro_models::JsonObject::new().into(),
        thought_signature: None,
    })];
    run_agent_loop_continue(context, &config(), emit, responses(vec![calls]))
        .await
        .unwrap();
    let entries = history.read().unwrap();
    let maestro_agent::AgentMessage::ToolResult(stored) = &entries[2] else {
        panic!("tool history");
    };
    assert_eq!(
        stored.read().unwrap().details,
        Some(serde_json::json!({"edited":true}))
    );
    assert!(Arc::ptr_eq(
        stored,
        retained.lock().unwrap().as_ref().unwrap()
    ));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn tool_lookup_uses_first_exact_name() {
    run(support::corpus::replay(&[
        "case-sensitive-tool",
        "tools-absent",
        "tools-empty",
        "tools-duplicates",
        "tool-unknown",
    ]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn invalid_tool_arguments_become_error_results() {
    run(support::corpus::replay(&["validation-error"]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn execution_failure_becomes_error_result() {
    run(support::corpus::replay(&["execute-error"]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn termination_requires_every_nonempty_result() {
    run(support::corpus::replay(&[
        "terminate-false",
        "terminate-true",
        "termination-mixed-absent-true",
        "termination-mixed-true-false",
        "termination-mixed-true-absent",
    ]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn progress_updates_overlap_and_settle_before_finalization() {
    run(overlapping_progress_case());
    run(support::corpus::replay(&["progress-2"]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn progress_failure_wins_over_execution_failure() {
    run(support::corpus::replay(&[
        "progress-sink-error",
        "execute-and-progress-error",
    ]));
}

/// Started progress listeners settle before a failure leaves the operation.
#[test]
fn progress_failure_settles_started_siblings_before_return() {
    run(progress_failure_case());
}
/// Use named readiness and completion witnesses without scheduling delays.
async fn progress_failure_case() {
    use futures_util::FutureExt;
    use std::sync::atomic::{AtomicBool, Ordering};
    let (release, released) = tokio::sync::oneshot::channel();
    let released = Arc::new(Mutex::new(Some(released)));
    let effect = Arc::new(AtomicBool::new(false));
    let observed_effect = Arc::clone(&effect);
    let ended = Arc::new(AtomicBool::new(false));
    let observed_end = Arc::clone(&ended);
    let emit = Arc::new(move |event| {
        progress_failure_sink(event, &released, &observed_effect, &observed_end)
    });
    let context = with_execute(Arc::new(|_, _, _, update| {
        let update = update.unwrap();
        update(AgentToolResult {
            details: Some(serde_json::json!(0)),
            ..result()
        });
        update(AgentToolResult {
            details: Some(serde_json::json!(1)),
            ..result()
        });
        ready(Err(diagnostic("execution failed")))
    }));
    let config = config();
    let mut operation = Box::pin(run_agent_loop_continue(
        context,
        &config,
        emit,
        responses(vec![tool_call()]),
    ));
    assert!(
        operation.as_mut().now_or_never().is_none(),
        "started sibling must remain awaited"
    );
    assert!(!effect.load(Ordering::SeqCst));
    release.send(()).unwrap();
    let error = operation.await.err().unwrap();
    assert_eq!(error.message, "progress second");
    assert!(effect.load(Ordering::SeqCst));
    assert!(!ended.load(Ordering::SeqCst));
}
/// Hold the first listener and fail the second before the first settles.
fn progress_failure_sink(
    event: maestro_agent::AgentEvent,
    gate: &Arc<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
    effect: &Arc<std::sync::atomic::AtomicBool>,
    ended: &Arc<std::sync::atomic::AtomicBool>,
) -> maestro_models::BoxFuture<Result<(), maestro_models::DiagnosticErrorInfo>> {
    use maestro_agent::AgentEvent;
    use std::sync::atomic::Ordering;
    match event {
        AgentEvent::ToolExecutionUpdate { partial_result, .. }
            if partial_result.details == Some(serde_json::json!(0)) =>
        {
            let gate = gate.lock().unwrap().take().unwrap();
            let effect = Arc::clone(effect);
            Box::pin(async move {
                gate.await.unwrap();
                effect.store(true, Ordering::SeqCst);
                Err(diagnostic("progress first"))
            })
        }
        AgentEvent::ToolExecutionUpdate { .. } => ready(Err(diagnostic("progress second"))),
        AgentEvent::ToolExecutionEnd { .. } | AgentEvent::AgentEnd { .. } => {
            ended.store(true, Ordering::SeqCst);
            ready(Ok(()))
        }
        _ => ready(Ok(())),
    }
}
/// Produce nonsecret controlled callback failures.
fn diagnostic(message: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo::error(message)
}

/// A second progress future releases the first before tool finalization.
async fn overlapping_progress_case() {
    use futures_util::FutureExt;
    use std::sync::atomic::{AtomicUsize, Ordering};
    let (release, released) = tokio::sync::oneshot::channel();
    let release = Arc::new(Mutex::new(Some(release)));
    let released = Arc::new(Mutex::new(Some(released)));
    let effects = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&effects);
    let emit = Arc::new(move |event| overlap_sink(event, &release, &released, &observed));
    let context = with_execute(Arc::new(|_, _, _, update| {
        let update = update.unwrap();
        update(AgentToolResult {
            details: Some(serde_json::json!(0)),
            ..result()
        });
        update(AgentToolResult {
            details: Some(serde_json::json!(1)),
            ..result()
        });
        ready(Ok(result()))
    }));
    let config = config();
    let mut operation = Box::pin(run_agent_loop_continue(
        context,
        &config,
        emit,
        responses(vec![tool_call()]),
    ));
    let completed = operation.as_mut().now_or_never();
    assert!(
        effects.load(Ordering::SeqCst) >= 1,
        "second progress future must release the first"
    );
    if let Some(result) = completed {
        assert!(result.is_ok());
    } else {
        assert!(operation.await.is_ok());
    }
    assert_eq!(effects.load(Ordering::SeqCst), 2);
}
/// Reentrant progress effects operate without serial callback-future dispatch.
fn overlap_sink(
    event: maestro_agent::AgentEvent,
    release: &Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    released: &Arc<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
    effects: &Arc<std::sync::atomic::AtomicUsize>,
) -> maestro_models::BoxFuture<Result<(), maestro_models::DiagnosticErrorInfo>> {
    use maestro_agent::AgentEvent;
    use std::sync::atomic::Ordering;
    match event {
        AgentEvent::ToolExecutionUpdate { partial_result, .. }
            if partial_result.details == Some(serde_json::json!(0)) =>
        {
            let gate = released.lock().unwrap().take().unwrap();
            let effects = Arc::clone(effects);
            Box::pin(async move {
                gate.await.unwrap();
                effects.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }
        AgentEvent::ToolExecutionUpdate { .. } => {
            let gate = release.lock().unwrap().take().unwrap();
            let effects = Arc::clone(effects);
            Box::pin(async move {
                gate.send(()).unwrap();
                effects.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }
        AgentEvent::ToolExecutionEnd { .. } => {
            assert_eq!(effects.load(Ordering::SeqCst), 2);
            ready(Ok(()))
        }
        _ => ready(Ok(())),
    }
}

/// Construct distinguishable, repeated calls in source order.
fn ordered_calls() -> maestro_models::SharedAssistantMessage {
    let calls = assistant("calls");
    calls.write().unwrap().content = ["zulu", "alpha", "zulu"]
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            AssistantContent::ToolCall(ToolCall {
                id: index.to_string(),
                name: name.into(),
                arguments: maestro_models::JsonObject::new().into(),
                thought_signature: None,
            })
        })
        .collect();
    calls
}

/// Artifact timestamp acquisition follows the execution-end completion barrier.
async fn artifact_timestamp_case() {
    use futures_util::FutureExt;
    let (release, released) = tokio::sync::oneshot::channel();
    let released = Arc::new(Mutex::new(Some(released)));
    let floor = Arc::new(Mutex::new(0.0));
    let observed = Arc::clone(&floor);
    let emit = Arc::new(move |event| {
        if !matches!(event, maestro_agent::AgentEvent::ToolExecutionEnd { .. }) {
            return ready(Ok(()));
        }
        let gate = released.lock().unwrap().take().unwrap();
        let observed = Arc::clone(&observed);
        Box::pin(async move {
            gate.await.unwrap();
            *observed.lock().unwrap() = maestro_models::timestamp_now();
            Ok(())
        }) as maestro_models::BoxFuture<_>
    });
    let context = with_execute(Arc::new(|_, _, _, _| ready(Ok(result()))));
    let config = config();
    let mut operation = Box::pin(run_agent_loop_continue(
        context,
        &config,
        emit,
        responses(vec![tool_call()]),
    ));
    assert!(operation.as_mut().now_or_never().is_none());
    let paused_at = maestro_models::timestamp_now();
    while maestro_models::timestamp_now() <= paused_at {
        std::thread::yield_now();
    }
    release.send(()).unwrap();
    let returned = operation.await.unwrap();
    let maestro_agent::AgentMessage::ToolResult(artifact) = &returned[1] else {
        panic!("tool artifact");
    };
    assert!(artifact.read().unwrap().timestamp >= *floor.lock().unwrap());
    assert!(artifact.read().unwrap().timestamp <= maestro_models::timestamp_now());
}

/// Construct the two executable declarations for the ordered-call witness.
fn ordered_tools(
    trace: &Arc<Mutex<Vec<(&'static str, String)>>>,
) -> Vec<maestro_agent::SharedAgentTool> {
    ["zulu", "alpha"]
        .into_iter()
        .map(|name| {
            let trace = Arc::clone(trace);
            Arc::new(RwLock::new(AgentTool {
                description: None,
                definition: Tool {
                    name: name.into(),
                    description: name.into(),
                    parameters: serde_json::json!({"type":"object"}).into(),
                },
                label: name.into(),
                prepare_arguments: None,
                execution_mode: None,
                execute: Arc::new(move |id, _, _, _| {
                    trace.lock().unwrap().push((name, id));
                    Box::pin(async {
                        Ok(AgentToolResult {
                            content: vec![],
                            details: Some(serde_json::Value::Null),
                            terminate: None,
                        })
                    })
                }),
            }))
        })
        .collect()
}

/// A synchronous observer already entered must settle before tool finalization.
#[test]
fn reserved_progress_waits_for_synchronous_observer_and_future() {
    run(reserved_progress_case());
}

/// Hold the callback thread inside the observer while execution completes.
async fn reserved_progress_case() {
    use futures_util::FutureExt;
    use std::sync::{
        Barrier,
        atomic::{AtomicBool, Ordering},
    };
    let gate = Arc::new(Barrier::new(2));
    let effect = Arc::new(AtomicBool::new(false));
    let ended = Arc::new(AtomicBool::new(false));
    let (entered, entry) = std::sync::mpsc::channel();
    let observer_gate = Arc::clone(&gate);
    let observer_effect = Arc::clone(&effect);
    let observer_end = Arc::clone(&ended);
    let emit = Arc::new(move |event| {
        reserved_progress_sink(
            &event,
            &entered,
            &observer_gate,
            &observer_effect,
            &observer_end,
        )
    });
    let entry = Arc::new(Mutex::new(entry));
    let thread = Arc::new(Mutex::new(None));
    let callback_thread = Arc::clone(&thread);
    let context = with_execute(Arc::new(move |_, _, _, update| {
        *callback_thread.lock().unwrap() =
            Some(std::thread::spawn(move || update.unwrap()(result())));
        entry.lock().unwrap().recv().unwrap();
        ready(Ok(result()))
    }));
    let config = config();
    let mut operation = Box::pin(run_agent_loop_continue(
        context,
        &config,
        emit,
        responses(vec![tool_call()]),
    ));
    let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        operation.as_mut().now_or_never()
    }));
    let ended_before_release = ended.load(Ordering::SeqCst);
    gate.wait();
    thread.lock().unwrap().take().unwrap().join().unwrap();
    let first = first.unwrap();
    assert!(
        first.is_none(),
        "reserved synchronous observer must remain awaited"
    );
    assert!(!ended_before_release);
    operation.await.unwrap();
    assert!(effect.load(Ordering::SeqCst));
    assert!(ended.load(Ordering::SeqCst));
}

/// Gate synchronous entry and assert the awaited effect at both end observations.
fn reserved_progress_sink(
    event: &maestro_agent::AgentEvent,
    entered: &std::sync::mpsc::Sender<()>,
    gate: &std::sync::Barrier,
    effect: &Arc<std::sync::atomic::AtomicBool>,
    ended: &std::sync::atomic::AtomicBool,
) -> maestro_models::BoxFuture<Result<(), maestro_models::DiagnosticErrorInfo>> {
    use maestro_agent::AgentEvent;
    use std::sync::atomic::Ordering;
    match event {
        AgentEvent::ToolExecutionUpdate { .. } => {
            entered.send(()).unwrap();
            gate.wait();
            let effect = Arc::clone(effect);
            Box::pin(async move {
                effect.store(true, Ordering::SeqCst);
                Ok(())
            }) as maestro_models::BoxFuture<_>
        }
        AgentEvent::ToolExecutionEnd { .. } | AgentEvent::AgentEnd { .. } => {
            assert!(
                effect.load(Ordering::SeqCst),
                "progress effect precedes end"
            );
            ended.store(true, Ordering::SeqCst);
            ready(Ok(()))
        }
        _ => ready(Ok(())),
    }
}

/// Waking the caller while closing progress must not hold admission.
#[test]
fn closing_progress_wakes_caller_without_holding_admission() {
    wake_reentry_case();
}

/// Waker that invokes the retained update callback from another thread and reports completion.
struct ReentrantWaker {
    callback: Arc<Mutex<Option<maestro_agent::AgentToolUpdateCallback>>>,
    armed: std::sync::atomic::AtomicBool,
    reentered: std::sync::atomic::AtomicBool,
}

impl std::task::Wake for ReentrantWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        if !self.armed.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        let Some(callback) = self.callback.lock().unwrap().clone() else {
            return;
        };
        std::thread::scope(|scope| {
            let worker = scope.spawn(move || callback(result()));
            worker.join().unwrap();
            self.reentered
                .store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }
}

/// Close progress with a caller waker that re-enters the retained callback.
fn wake_reentry_case() {
    use std::{pin::Pin, task::Context};
    let callback = Arc::new(Mutex::new(None));
    let retained = Arc::clone(&callback);
    let (release, released) = tokio::sync::oneshot::channel::<()>();
    let released = Arc::new(Mutex::new(Some(released)));
    let context = with_execute(Arc::new(move |_, _, _, update| {
        *retained.lock().unwrap() = update;
        let gate = released.lock().unwrap().take().unwrap();
        Box::pin(async move {
            gate.await.unwrap();
            Ok(result())
        })
    }));
    let config = config();
    let mut operation = Box::pin(run_agent_loop_continue(
        context,
        &config,
        Arc::new(|_| ready(Ok(()))),
        responses(vec![tool_call()]),
    ));
    let waker = Arc::new(ReentrantWaker {
        callback,
        armed: std::sync::atomic::AtomicBool::new(false),
        reentered: std::sync::atomic::AtomicBool::new(false),
    });
    let handle = std::task::Waker::from(Arc::clone(&waker));
    let mut cx = Context::from_waker(&handle);
    assert!(Pin::new(&mut operation).poll(&mut cx).is_pending());
    release.send(()).unwrap();
    waker.armed.store(true, std::sync::atomic::Ordering::SeqCst);
    let _ = Pin::new(&mut operation).poll(&mut cx);
    assert!(
        waker.reentered.load(std::sync::atomic::Ordering::SeqCst),
        "retained callback must not block on admission while closing wakes the caller"
    );
}
