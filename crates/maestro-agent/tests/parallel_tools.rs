//! Concurrent tool batches through awaited conversation operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{AgentEvent, AgentMessage, ToolExecutionMode as Mode, run_agent_loop};
use std::sync::{Arc, Mutex};
use support::*;

/// Finalizers overlap while artifacts retain source order.
#[test]
fn parallel_finalizers_complete_before_source_ordered_artifacts() {
    run(async {
        finalizer_case().await;
        after_overlap_case().await;
        artifact_timestamp_case().await;
        support::corpus::replay_controls(&["duplicate-call-ids"]).await;
    });
}
/// Release an earlier call from a later finalizer in the same batch.
async fn finalizer_case() {
    use futures_util::FutureExt;
    let (release, wait) = tokio::sync::oneshot::channel();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let release = Arc::new(Mutex::new(Some(release)));
    let context = with_execute(overlapping_execute(wait, release));
    let calls = two_calls();
    let ends = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&ends);
    let mut config = config();
    config.tool_execution = None;
    let mut operation = Box::pin(run_agent_loop(
        vec![],
        context,
        &config,
        Arc::new(move |e| {
            match e {
                AgentEvent::ToolExecutionEnd { tool_call, .. } => {
                    captured.lock().unwrap().push(tool_call.id);
                }
                AgentEvent::MessageStart {
                    message: AgentMessage::ToolResult(_),
                } => assert_eq!(captured.lock().unwrap().len(), 2),
                _ => (),
            }
            ready(Ok(()))
        }),
        responses(vec![calls]),
    ));
    let completion = operation.as_mut().now_or_never();
    let reached_second = ends.lock().unwrap().contains(&"second".to_owned());
    // Close the observation set even if a sequential implementation failed to enter the second call.
    if !reached_second {
        drop(operation);
        panic!("second call must release first without an external driver");
    }
    let messages = match completion {
        Some(value) => value,
        None => operation.await,
    }
    .unwrap();
    assert_eq!(*ends.lock().unwrap(), ["second", "first"]);
    let ids: Vec<_> = messages
        .iter()
        .filter_map(|m| match m {
            AgentMessage::ToolResult(r) => Some(r.read().unwrap().tool_call_id.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["first", "second"]);
}
/// Construct two distinguishable calls with one declaration.
fn two_calls() -> maestro_models::SharedAssistantMessage {
    let message = tool_call();
    let mut m = message.write().unwrap();
    let maestro_models::AssistantContent::ToolCall(c) = &mut m.content[0] else {
        panic!("call")
    };
    c.id = "first".into();
    let mut second = c.clone();
    second.id = "second".into();
    m.content
        .push(maestro_models::AssistantContent::ToolCall(second));
    drop(m);
    message
}

/// Build two executions with an explicit same-operation completion dependency.
fn overlapping_execute(
    wait: Arc<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
    release: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
) -> maestro_agent::ExecuteTool {
    Arc::new(move |id, _, _, _| {
        let wait = Arc::clone(&wait);
        let release = Arc::clone(&release);
        Box::pin(async move {
            if id == "first" {
                let rx = wait.lock().unwrap().take().unwrap();
                rx.await.unwrap();
            } else {
                release.lock().unwrap().take().unwrap().send(()).unwrap();
            }
            Ok(result())
        })
    })
}

/// Compare complete public observations for default parallel preflights before any execution.
#[test]
fn default_parallel_preflights_before_any_execution() {
    run(async {
        support::corpus::replay_controls(&["defaults", "explicit-parallel"]).await;
        preflight_gate_case().await;
    });
}

/// Compare complete public observations for sequential mode and first matching override serialize batches.
#[test]
fn sequential_mode_and_first_matching_override_serialize_batches() {
    run(async {
        fixed_mode_case().await;
        support::corpus::replay_controls(&[
            "explicit-sequential",
            "tool-mode-sequential",
            "duplicate-first-sequential",
            "mixed-tool-mode",
            "global-sequential-overrides-parallel-tools",
        ])
        .await;
    });
}

/// Compare complete public observations for termination uses every finalized result.
#[test]
fn termination_uses_every_finalized_result() {
    run(support::corpus::replay_controls(&[
        "terminate-parallel-50",
        "terminate-parallel-51",
        "terminate-parallel-52",
        "terminate-parallel-53",
        "terminate-parallel-54",
        "terminate-parallel-55",
        "terminate-parallel-56",
        "terminate-sequential-57",
        "terminate-sequential-58",
        "terminate-sequential-59",
        "terminate-sequential-60",
        "terminate-sequential-61",
        "terminate-sequential-62",
        "terminate-sequential-63",
        "after-termination-64",
        "after-termination-65",
        "empty-call-batch",
    ]));
}

/// Compare complete public observations for immediate failures end during preflight.
#[test]
fn immediate_failures_end_during_preflight() {
    run(async {
        immediate_sink_failure_case().await;
        support::corpus::replay_controls(&["unknown-with-allowed"]).await;
    });
}

/// Explicit parallel declarations preserve execution overlap.
#[test]
fn parallel_tool_declarations_allow_overlap() {
    run(async {
        support::corpus::replay_controls(&["tool-mode-parallel", "duplicate-first-parallel"]).await;
        let (release, wait) = tokio::sync::oneshot::channel();
        let context = with_execute(overlapping_execute(
            Arc::new(Mutex::new(Some(wait))),
            Arc::new(Mutex::new(Some(release))),
        ));
        context.tools.as_ref().unwrap().read().unwrap()[0]
            .write()
            .unwrap()
            .execution_mode = Some(maestro_agent::ToolExecutionMode::Parallel);
        let mut config = config();
        config.tool_execution = None;
        let returned = run_agent_loop(
            vec![],
            context,
            &config,
            Arc::new(|_| ready(Ok(()))),
            responses(vec![two_calls()]),
        )
        .await
        .unwrap();
        assert_eq!(returned.len(), 3);
    });
}

/// Execution settlement cannot discard progress observers admitted on two threads.
#[test]
fn parallel_progress_admission_settles_both_closing_tools() {
    run(admission_case());
}
/// Hold both synchronous observers across the same batch's execution completion.
async fn admission_case() {
    use futures_util::FutureExt;
    let gates = Arc::new([std::sync::Barrier::new(2), std::sync::Barrier::new(2)]);
    let (entered, entry) = std::sync::mpsc::channel();
    let entry = Arc::new(Mutex::new(entry));
    let threads = Arc::new(Mutex::new(Vec::new()));
    let callbacks = Arc::new(Mutex::new(Vec::new()));
    let effects = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&effects);
    let captured_threads = Arc::clone(&threads);
    let captured_callbacks = Arc::clone(&callbacks);
    let context = with_execute(Arc::new(move |_, _, _, update| {
        let update = update.unwrap();
        captured_callbacks.lock().unwrap().push(Arc::clone(&update));
        captured_threads
            .lock()
            .unwrap()
            .push(std::thread::spawn(move || update(result())));
        entry.lock().unwrap().recv().unwrap();
        ready(Ok(result()))
    }));
    let mut config = config();
    config.tool_execution = None;
    let sink = admission_sink(Arc::clone(&gates), entered, observed);
    let mut operation = Box::pin(run_agent_loop(
        vec![],
        context,
        &config,
        sink,
        responses(vec![two_calls()]),
    ));
    let first = operation.as_mut().now_or_never();
    let pending_with_both_reserved = first.is_none();
    gates[0].wait();
    join_observer(&threads);
    let second_poll = if first.is_none() {
        operation.as_mut().now_or_never()
    } else {
        None
    };
    let pending_with_one_reserved = second_poll.is_none();
    gates[1].wait();
    join_observer(&threads);
    let returned = finish(first.or(second_poll), operation).await.unwrap();
    assert!(
        pending_with_both_reserved,
        "batch must await both reserved observers"
    );
    assert!(
        pending_with_one_reserved,
        "batch must await remaining reserved observer"
    );
    assert_eq!(*effects.lock().unwrap(), [0, 1]);
    assert_eq!(returned.len(), 3);
    let retained = std::mem::take(&mut *callbacks.lock().unwrap());
    for callback in retained {
        callback(result());
    }
    assert_eq!(*effects.lock().unwrap(), [0, 1]);
}

/// The first completion failure waits for siblings and suppresses batch artifacts.
#[test]
fn parallel_failure_settles_siblings_before_return() {
    run(async {
        for progress_failure in [true, false] {
            failure_case(progress_failure).await;
        }
    });
}
/// Fail the second source call before releasing the first admitted execution.
async fn failure_case(progress_failure: bool) {
    use futures_util::FutureExt;
    let (release, wait) = tokio::sync::oneshot::channel();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let effects = Arc::new(Mutex::new(Vec::new()));
    let executed = Arc::clone(&effects);
    let context = with_execute(Arc::new(move |id, _, _, update| {
        let wait = Arc::clone(&wait);
        let executed = Arc::clone(&executed);
        Box::pin(async move {
            if id == "first" {
                let wait = wait.lock().unwrap().take().unwrap();
                wait.await.unwrap();
                executed.lock().unwrap().push("sibling-effect".to_owned());
            } else if progress_failure {
                update.unwrap()(result());
            }
            Ok(result())
        })
    }));
    let mut config = config();
    config.tool_execution = None;
    let finalized = Arc::clone(&effects);
    config.after_tool_call = Some(Arc::new(move |c, _| {
        finalized
            .lock()
            .unwrap()
            .push(format!("after:{}", c.tool_call.id));
        Box::pin(async { Ok(None) })
    }));
    let sink = failure_sink(Arc::clone(&effects), progress_failure);
    let mut operation = Box::pin(run_agent_loop(
        vec![],
        context,
        &config,
        sink,
        responses(vec![two_calls()]),
    ));
    let first = operation.as_mut().now_or_never();
    let pending = first.is_none();
    let _ = release.send(());
    let error = finish(first, operation).await.err().unwrap();
    assert!(pending, "failed batch must wait for admitted sibling");
    assert_eq!(
        error.message,
        if progress_failure {
            "second progress failed"
        } else {
            "second end failed"
        }
    );
    let effects = effects.lock().unwrap();
    assert!(effects.iter().any(|e| e == "sibling-effect"));
    assert!(effects.iter().any(|e| e == "end:first"));
    assert_eq!(
        effects.iter().any(|e| e == "after:second"),
        !progress_failure
    );
}
/// Controlled operation failure, without credential-bearing diagnostics.
fn failure_diagnostic(message: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo::new(None, message)
}

/// Selected records survive removal and their execute slots remain live until use.
#[test]
fn selected_tools_survive_inventory_edits_until_finalization() {
    run(selection_case());
}
/// Replace the inventory during the first preflight and observe both selections.
async fn selection_case() {
    let context = with_execute(Arc::new(|_, _, _, _| panic!("stale execution slot")));
    let selected = Arc::clone(&context.tools.as_ref().unwrap().read().unwrap()[0]);
    let weak = Arc::downgrade(&selected);
    drop(selected);
    let lifetime = Arc::new(());
    let capture = Arc::downgrade(&lifetime);
    let mut config = config();
    config.tool_execution = None;
    config.before_tool_call = Some(selection_hook(weak.clone(), lifetime));
    let retained = weak.clone();
    config.after_tool_call = Some(Arc::new(move |c, _| {
        if c.tool_call.id == "first" {
            assert!(retained.upgrade().is_some());
        }
        Box::pin(async { Ok(None) })
    }));
    let progress = weak.clone();
    let returned = run_agent_loop(
        vec![],
        context,
        &config,
        Arc::new(move |e| {
            if matches!(e, AgentEvent::ToolExecutionUpdate { .. }) {
                assert!(progress.upgrade().is_some());
            }
            ready(Ok(()))
        }),
        responses(vec![two_calls()]),
    )
    .await
    .unwrap();
    let details: Vec<_> = returned
        .iter()
        .filter_map(|m| match m {
            AgentMessage::ToolResult(r) => r.read().unwrap().details.clone(),
            _ => None,
        })
        .collect();
    assert_eq!(
        details,
        [
            serde_json::json!("selected edit"),
            serde_json::json!("replacement")
        ]
    );
    assert!(weak.upgrade().is_none());
    config.before_tool_call = None;
    assert!(capture.upgrade().is_none());
}

/// Pause synchronous progress observation before admitting its returned future.
fn admission_sink(
    gates: Arc<[std::sync::Barrier; 2]>,
    entered: std::sync::mpsc::Sender<()>,
    observed: Arc<Mutex<Vec<usize>>>,
) -> maestro_agent::AgentEventSink {
    Arc::new(move |event| {
        if let AgentEvent::ToolExecutionUpdate { tool_call, .. } = event {
            let index = usize::from(tool_call.id == "second");
            entered.send(()).unwrap();
            gates[index].wait();
            let observed = Arc::clone(&observed);
            return Box::pin(async move {
                observed.lock().unwrap().push(index);
                Ok(())
            }) as maestro_models::BoxFuture<_>;
        }
        ready(Ok(()))
    })
}

/// Observe completed siblings while injecting distinct completion failures.
fn failure_sink(
    observed: Arc<Mutex<Vec<String>>>,
    progress_failure: bool,
) -> maestro_agent::AgentEventSink {
    Arc::new(move |event| {
        let failure = match event {
            AgentEvent::ToolExecutionUpdate { .. } if progress_failure => {
                Some("second progress failed")
            }
            AgentEvent::ToolExecutionEnd { tool_call, .. } => {
                observed
                    .lock()
                    .unwrap()
                    .push(format!("end:{}", tool_call.id));
                if progress_failure {
                    None
                } else if tool_call.id == "second" {
                    Some("second end failed")
                } else {
                    Some("first end failed")
                }
            }
            AgentEvent::MessageStart {
                message: AgentMessage::ToolResult(_),
            }
            | AgentEvent::TurnEnd { .. }
            | AgentEvent::AgentEnd { .. } => panic!("failed batch must not synthesize completion"),
            _ => None,
        };
        ready(failure.map_or(Ok(()), |message| Err(failure_diagnostic(message))))
    })
}

/// Edit the retained execute slot and replace only its inventory entry.
fn selection_hook(
    retained: std::sync::Weak<std::sync::RwLock<maestro_agent::AgentTool>>,
    lifetime: Arc<()>,
) -> maestro_agent::BeforeToolCall<maestro_agent::NoCustomMessages> {
    Arc::new(move |c, _| {
        if c.tool_call.id == "first" {
            retained.upgrade().unwrap().write().unwrap().execute =
                selected_execute(Arc::clone(&lifetime));
            let replacement = with_execute(Arc::new(|_, _, _, _| {
                ready(Ok(maestro_agent::AgentToolResult {
                    details: Some(serde_json::json!("replacement")),
                    ..result()
                }))
            }));
            let replacement = replacement.tools.unwrap().write().unwrap().pop().unwrap();
            *c.context.tools.as_ref().unwrap().write().unwrap() = vec![replacement];
        }
        Box::pin(async { Ok(None) })
    })
}

/// Retain the capture across both synchronous progress and awaited execution.
fn selected_execute(lifetime: Arc<()>) -> maestro_agent::ExecuteTool {
    Arc::new(move |_, _, _, update| {
        let lifetime = Arc::clone(&lifetime);
        update.unwrap()(result());
        Box::pin(async move {
            drop(lifetime);
            Ok(maestro_agent::AgentToolResult {
                details: Some(serde_json::json!("selected edit")),
                ..result()
            })
        })
    })
}

/// A later before-hook blocks every execute callback until preflight settles.
async fn preflight_gate_case() {
    use futures_util::FutureExt;
    let trace = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&trace);
    let context = with_execute(Arc::new(move |id, _, _, _| {
        captured.lock().unwrap().push(format!("execute:{id}"));
        ready(Ok(result()))
    }));
    let captured = Arc::clone(&trace);
    context.tools.as_ref().unwrap().read().unwrap()[0]
        .write()
        .unwrap()
        .prepare_arguments = Some(Arc::new(move |args| {
        captured.lock().unwrap().push("prepare".into());
        Ok(args.clone())
    }));
    let (release, wait) = tokio::sync::oneshot::channel();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let mut config = config();
    config.tool_execution = None;
    let captured = Arc::clone(&trace);
    config.before_tool_call = Some(Arc::new(move |c, _| {
        captured
            .lock()
            .unwrap()
            .push(format!("before:{}", c.tool_call.id));
        let wait = if c.tool_call.id == "second" {
            wait.lock().unwrap().take()
        } else {
            None
        };
        Box::pin(async move {
            if let Some(wait) = wait {
                wait.await.unwrap();
            }
            Ok(None)
        })
    }));
    let mut operation = Box::pin(run_agent_loop(
        vec![],
        context,
        &config,
        Arc::new(|_| ready(Ok(()))),
        responses(vec![two_calls()]),
    ));
    let pending = operation.as_mut().now_or_never().is_none();
    let observed = trace.lock().unwrap().clone();
    release.send(()).unwrap();
    let returned = operation.await.unwrap();
    assert!(pending, "later preflight must block the batch");
    assert_eq!(
        observed,
        ["prepare", "before:first", "prepare", "before:second"]
    );
    assert_eq!(
        trace.lock().unwrap().join(","),
        "prepare,before:first,prepare,before:second,execute:first,execute:second"
    );
    assert_eq!(returned.len(), 3);
}

/// A later after-hook releases an earlier finalizer without an external driver.
async fn after_overlap_case() {
    use futures_util::FutureExt;
    let (release, wait) = tokio::sync::oneshot::channel();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let release = Arc::new(Mutex::new(Some(release)));
    let context = with_execute(Arc::new(|_, _, _, _| ready(Ok(result()))));
    let mut config = config();
    config.tool_execution = None;
    config.after_tool_call = Some(Arc::new(move |c, _| {
        let wait = if c.tool_call.id == "first" {
            wait.lock().unwrap().take()
        } else {
            release.lock().unwrap().take().unwrap().send(()).unwrap();
            None
        };
        Box::pin(async move {
            if let Some(wait) = wait {
                wait.await.unwrap();
            }
            Ok(None)
        })
    }));
    let ends = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&ends);
    let mut operation = Box::pin(run_agent_loop(
        vec![],
        context,
        &config,
        Arc::new(move |e| {
            if let AgentEvent::ToolExecutionEnd { tool_call, .. } = e {
                captured.lock().unwrap().push(tool_call.id);
            }
            ready(Ok(()))
        }),
        responses(vec![two_calls()]),
    ));
    let first = operation.as_mut().now_or_never();
    let reached_second = ends.lock().unwrap().contains(&"second".into());
    if !reached_second {
        drop(operation);
        panic!("after finalizers must overlap");
    }
    let returned = match first {
        Some(value) => value,
        None => operation.await,
    }
    .unwrap();
    assert_eq!(*ends.lock().unwrap(), ["second", "first"]);
    assert_eq!(returned.len(), 3);
}
/// Assign timestamps and expose artifacts only after the last end observation.
async fn artifact_timestamp_case() {
    use futures_util::FutureExt;
    let context = with_execute(Arc::new(|_, _, _, _| ready(Ok(result()))));
    let history = Arc::clone(&context.messages);
    let mut config = config();
    config.tool_execution = None;
    let (release, wait) = tokio::sync::oneshot::channel();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let sink = timestamp_sink(Arc::clone(&history), wait);
    let mut operation = Box::pin(maestro_agent::run_agent_loop_continue(
        context,
        &config,
        sink,
        responses(vec![two_calls()]),
    ));
    let pending = operation.as_mut().now_or_never().is_none();
    let boundary = maestro_models::timestamp_now();
    while maestro_models::timestamp_now() <= boundary {
        std::thread::yield_now();
    }
    release.send(()).unwrap();
    let returned = operation.await.unwrap();
    assert!(pending, "last end observation must gate artifacts");
    for message in &returned[1..] {
        let AgentMessage::ToolResult(message) = message else {
            panic!("artifact");
        };
        assert!(
            message.read().unwrap().timestamp > boundary,
            "timestamp belongs to artifact emission phase"
        );
    }
    assert_eq!(history.read().unwrap().len(), 4);
}

/// Close the end-observation set before artifacts enter the retained history.
fn timestamp_sink(
    history_observed: Arc<std::sync::RwLock<Vec<AgentMessage>>>,
    wait: Arc<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
) -> maestro_agent::AgentEventSink {
    Arc::new(move |e| {
        if let AgentEvent::ToolExecutionEnd { tool_call, .. } = &e
            && tool_call.id == "second"
        {
            let wait = wait.lock().unwrap().take().unwrap();
            return Box::pin(async move {
                wait.await.unwrap();
                Ok(())
            }) as maestro_models::BoxFuture<_>;
        }
        if matches!(
            e,
            AgentEvent::MessageStart {
                message: AgentMessage::ToolResult(_)
            } | AgentEvent::MessageEnd {
                message: AgentMessage::ToolResult(_)
            }
        ) {
            assert!(
                !history_observed
                    .read()
                    .unwrap()
                    .iter()
                    .any(|m| matches!(m, AgentMessage::ToolResult(_)))
            );
        }
        ready(Ok(()))
    })
}

/// Preserve an early result while joining the operation when it remained pending.
async fn finish<T>(first: Option<T>, operation: impl std::future::Future<Output = T>) -> T {
    match first {
        Some(value) => value,
        None => operation.await,
    }
}

/// Join a reserved observer without retaining the handle-collection lock.
fn join_observer(threads: &Mutex<Vec<std::thread::JoinHandle<()>>>) {
    let thread = threads.lock().unwrap().remove(0);
    thread.join().unwrap();
}

/// Batch mode is captured before caller start observations edit the inventory.
async fn fixed_mode_case() {
    let context = with_execute(Arc::new(|_, _, _, _| ready(Ok(result()))));
    let inventory = Arc::clone(context.tools.as_ref().unwrap());
    inventory.read().unwrap()[0].write().unwrap().execution_mode =
        Some(maestro_agent::ToolExecutionMode::Sequential);
    let trace = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&trace);
    let sink = Arc::new(move |event| {
        let entry = match event {
            AgentEvent::ToolExecutionStart { tool_call } => {
                inventory.read().unwrap()[0].write().unwrap().execution_mode =
                    Some(maestro_agent::ToolExecutionMode::Parallel);
                Some(format!("start:{}", tool_call.id))
            }
            AgentEvent::ToolExecutionEnd { tool_call, .. } => Some(format!("end:{}", tool_call.id)),
            AgentEvent::MessageEnd {
                message: AgentMessage::ToolResult(result),
            } => Some(format!("artifact:{}", result.read().unwrap().tool_call_id)),
            _ => None,
        };
        if let Some(entry) = entry {
            observed.lock().unwrap().push(entry);
        }
        ready(Ok(()))
    });
    let mut config = config();
    config.tool_execution = None;
    let returned = run_agent_loop(vec![], context, &config, sink, responses(vec![two_calls()]))
        .await
        .unwrap();
    assert_eq!(
        trace.lock().unwrap().join(","),
        "start:first,end:first,artifact:first,start:second,end:second,artifact:second"
    );
    assert_eq!(returned.len(), 3);
}

/// A failed start or immediate error end aborts preflight before allowed execution.
async fn immediate_sink_failure_case() {
    for fail_start in [true, false] {
        let context = with_execute(Arc::new(|_, _, _, _| {
            panic!("failed preflight sink must prevent execution")
        }));
        let calls = two_calls();
        if let maestro_models::AssistantContent::ToolCall(call) =
            &mut calls.write().unwrap().content[0]
        {
            call.name = "missing".into();
        }
        let mut config = config();
        config.tool_execution = None;
        config.after_tool_call = Some(Arc::new(|_, _| panic!("immediate errors skip after hooks")));
        let observed = Arc::new(Mutex::new(Vec::new()));
        let sink = immediate_failure_sink(Arc::clone(&observed), fail_start);
        let error = run_agent_loop(vec![], context, &config, sink, responses(vec![calls]))
            .await
            .err()
            .unwrap();
        assert_eq!(
            error.message,
            if fail_start {
                "start failed"
            } else {
                "immediate end failed"
            }
        );
        assert_eq!(
            observed.lock().unwrap().join(","),
            if fail_start {
                "start:first"
            } else {
                "start:first,end:first"
            }
        );
    }
}

/// Reject the selected preflight observation while retaining its prior effects.
fn immediate_failure_sink(
    captured: Arc<Mutex<Vec<String>>>,
    fail_start: bool,
) -> maestro_agent::AgentEventSink {
    Arc::new(move |event| {
        let outcome = match event {
            AgentEvent::ToolExecutionStart { tool_call } => {
                captured
                    .lock()
                    .unwrap()
                    .push(format!("start:{}", tool_call.id));
                if fail_start {
                    Err(failure_diagnostic("start failed"))
                } else {
                    Ok(())
                }
            }
            AgentEvent::ToolExecutionEnd { tool_call, .. } => {
                captured
                    .lock()
                    .unwrap()
                    .push(format!("end:{}", tool_call.id));
                Err(failure_diagnostic("immediate end failed"))
            }
            AgentEvent::AgentEnd { .. } | AgentEvent::TurnEnd { .. } => {
                panic!("sink failure must not synthesize completion")
            }
            _ => Ok(()),
        };
        ready(outcome)
    })
}

/// Where the signal aborts.
#[derive(Clone, Copy)]
enum AbortAt {
    /// Inside the first call's execution.
    Execution,
    /// Inside the first call's before hook.
    Hook,
}

/// The ids that started and executed, and each result's error flag, when the signal aborts
/// at `at` in a batch run under `mode`.
async fn aborted_batch(
    mode: maestro_agent::ToolExecutionMode,
    at: AbortAt,
) -> (Vec<String>, Vec<String>, Vec<bool>) {
    let in_hook = matches!(at, AbortAt::Hook);
    let signal = maestro_models::Cancellation::new();
    let executed = Arc::new(Mutex::new(Vec::new()));
    let (log, trigger) = (Arc::clone(&executed), signal.clone());
    let context = with_execute(Arc::new(move |id, _, _, _| {
        log.lock().unwrap().push(id);
        if !in_hook {
            trigger.abort();
        }
        ready(Ok(result()))
    }));
    let mut config = config();
    config.tool_execution = Some(mode);
    if in_hook {
        let trigger = signal.clone();
        config.before_tool_call = Some(Arc::new(move |_, _| {
            trigger.abort();
            ready(Ok(None))
        }));
    }
    let started = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&started);
    let mut options = responses(vec![two_calls()]);
    options.signal = Some(signal);
    let sink = Arc::new(move |event| {
        if let AgentEvent::ToolExecutionStart { tool_call } = event {
            seen.lock().unwrap().push(tool_call.id);
        }
        ready(Ok(()))
    });
    let messages = run_agent_loop(vec![], context, &config, sink, options)
        .await
        .unwrap();
    let errors = messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::ToolResult(result) => Some(result.read().unwrap().is_error),
            _ => None,
        })
        .collect();
    let (started, executed) = (
        started.lock().unwrap().clone(),
        executed.lock().unwrap().clone(),
    );
    (started, executed, errors)
}

/// An abort during a sequential call prepares no later sibling.
#[test]
fn sequential_abort_prepares_no_later_sibling() {
    let (started, executed, errors) = run(aborted_batch(Mode::Sequential, AbortAt::Execution));
    assert_eq!(started, ["first"]);
    assert_eq!(executed, ["first"]);
    assert_eq!(errors, [false]);
}

/// An abort raised in a before hook executes no tool and prepares no later sibling.
#[test]
fn abort_in_a_before_hook_executes_no_tool() {
    for mode in [Mode::Sequential, Mode::Parallel] {
        let (started, executed, errors) = run(aborted_batch(mode, AbortAt::Hook));
        assert_eq!(started, ["first"], "{mode:?}");
        assert!(executed.is_empty(), "{mode:?}");
        assert_eq!(errors, [true], "{mode:?}");
    }
}
