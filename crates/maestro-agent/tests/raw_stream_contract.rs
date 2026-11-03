//! Ownership, forwarding and settlement of observable raw operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{AgentEvent, AgentLoopOptions, AgentMessage, agent_loop, agent_loop_continue};
use maestro_models::{AssistantMessageEventStream, Cancellation, EventStream, StopReason};
use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Wake, Waker},
};
use support::{observable::*, *};

/// The capturing spawner retains one operation before request conversion runs.
#[test]
fn maestro_raw_stream_returns_before_work_is_driven() {
    for continuation in [false, true] {
        let tasks = Arc::new(owned::Tasks::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&calls);
        let mut config = config();
        let convert = Arc::clone(&config.convert_to_llm);
        config.convert_to_llm = Arc::new(move |entries| {
            count.fetch_add(1, Ordering::SeqCst);
            convert(entries)
        });
        let stream = start(
            continuation,
            context(),
            config,
            responses(vec![assistant("done")]),
            &tasks,
        );
        assert_eq!(tasks.0.lock().unwrap().len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        run(tasks.take());
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            drain(&stream).messages.len(),
            if continuation { 1 } else { 2 }
        );
    }
}
/// Prompt submission captures the outer history while retaining entry aliases.
#[test]
fn maestro_submission_captures_prompt_history() {
    let context = context();
    let history = Arc::clone(&context.messages);
    let old = user("old");
    *history.write().unwrap() = vec![old.clone()];
    let mut config = config();
    let retained = old.clone();
    let convert = Arc::clone(&config.convert_to_llm);
    config.convert_to_llm = Arc::new(move |entries| {
        let entries_read = entries.read().unwrap();
        assert_eq!(
            entries_read.iter().map(text).collect::<Vec<_>>(),
            ["edited", "prompt"]
        );
        same_entry(&entries_read[0], &old);
        drop(entries_read);
        convert(entries)
    });
    let tasks = Arc::new(owned::Tasks::default());
    let stream = start(
        false,
        context,
        config,
        responses(vec![assistant("done")]),
        &tasks,
    );
    let AgentMessage::User(entry) = &retained else {
        panic!("user expected")
    };
    entry.write().unwrap().content = maestro_models::UserContent::Text("edited".into());
    *history.write().unwrap() = vec![user("late")];
    run(tasks.take());
    assert_eq!(
        drain(&stream).messages.iter().map(text).collect::<Vec<_>>(),
        ["prompt", "done"]
    );
    assert_eq!(
        history.read().unwrap().iter().map(text).collect::<Vec<_>>(),
        ["late"]
    );
}

/// Already admitted continuations keep live history after another operation finishes.
#[test]
fn maestro_overlapping_continuations_keep_admission_and_live_history() {
    let context = context();
    let history = Arc::clone(&context.messages);
    let tasks = Arc::new(owned::Tasks::default());
    let config = Arc::new(config());
    let first = agent_loop_continue(
        context.clone(),
        Arc::clone(&config),
        responses(vec![assistant("first")]),
        tasks.spawner(),
    )
    .unwrap();
    let first_task = tasks.take();
    let mut options = responses(vec![assistant("second")]);
    let source = options.stream_fn.take().unwrap();
    options.stream_fn = Some(Arc::new(move |model, request, options, extra| {
        assert_eq!(request.messages.len(), 2);
        let maestro_models::Message::Assistant(previous) = &request.messages[1] else {
            panic!("live assistant missing")
        };
        assert_eq!(previous.content, assistant("first").read().unwrap().content);
        source(model, request, options, extra)
    }));
    let second = agent_loop_continue(context, config, options, tasks.spawner()).unwrap();
    run(first_task);
    assert_eq!(
        drain(&first).messages.iter().map(text).collect::<Vec<_>>(),
        ["first"]
    );
    run(tasks.take());
    let mut result = Box::pin(second.result());
    assert!(
        poll(result.as_mut()).is_ready(),
        "admitted continuation must settle"
    );
    assert_eq!(
        drain(&second).messages.iter().map(text).collect::<Vec<_>>(),
        ["second"]
    );
    assert_eq!(
        history.read().unwrap().iter().map(text).collect::<Vec<_>>(),
        ["earlier", "first", "second"]
    );
}

/// Absent and paused consumers do not delay production.
#[test]
fn maestro_slow_observers_do_not_block_production() {
    for paused in [false, true] {
        let tasks = Arc::new(owned::Tasks::default());
        let stream = start(
            false,
            context(),
            config(),
            responses(vec![assistant("done")]),
            &tasks,
        );
        let mut reader = Box::pin(stream.next());
        if paused {
            assert!(poll(reader.as_mut()).is_pending());
        }
        let mut task = tasks.take();
        assert!(poll(task.as_mut()).is_ready());
        let mut events = if paused {
            vec![kind(&run(reader).unwrap())]
        } else {
            drop(reader);
            vec![]
        };
        let recorded = drain(&stream);
        events.extend(recorded.events.iter().map(kind));
        assert_eq!(
            events,
            [
                "agent_start",
                "turn_start",
                "message_start",
                "message_end",
                "message_start",
                "message_end",
                "turn_end",
                "agent_end"
            ]
        );
        assert_eq!(
            recorded.messages.iter().map(text).collect::<Vec<_>>(),
            ["prompt", "done"]
        );
    }
}
/// Dropping observation does not cancel the retained producer or its tool effect.
#[test]
fn maestro_dropped_observers_preserve_owned_work() {
    for dropped in 0..3 {
        let effect = Arc::new(AtomicUsize::new(0));
        let count = Arc::clone(&effect);
        let signal = Cancellation::new();
        let context = with_execute(Arc::new(move |_, _, signal, _| {
            assert!(!signal.unwrap().is_aborted());
            count.fetch_add(1, Ordering::SeqCst);
            ready(Ok(result()))
        }));
        let tasks = Arc::new(owned::Tasks::default());
        let mut options = responses(vec![tool_call()]);
        options.signal = Some(signal.clone());
        let stream = start(false, context, config(), options, &tasks);
        match dropped {
            0 => drop(stream),
            1 => {
                let mut read = Box::pin(stream.next());
                assert!(poll(read.as_mut()).is_pending());
                drop(read);
                drop(stream);
            }
            _ => {
                let mut result = Box::pin(stream.result());
                assert!(poll(result.as_mut()).is_pending());
                drop(result);
                drop(stream);
            }
        }
        run(tasks.take());
        assert_eq!(effect.load(Ordering::SeqCst), 1);
        assert!(!signal.is_aborted());
        assert!(tasks.0.lock().unwrap().is_empty());
    }
}
/// A reader's terminal wake sees the result before successful producer return.
#[test]
fn maestro_terminal_result_keeps_message_handles() {
    let (stream, tasks, release) = held_terminal();
    let mut result = Box::pin(stream.result());
    assert!(poll(result.as_mut()).is_pending());
    let mut task = tasks.take();
    assert!(poll(task.as_mut()).is_pending());
    for expected in [
        "agent_start",
        "turn_start",
        "message_start",
        "message_end",
        "message_start",
        "message_end",
        "turn_end",
    ] {
        assert_eq!(kind(&run(stream.next()).unwrap()), expected);
        assert!(poll(result.as_mut()).is_pending());
    }
    let witness = Arc::new(TerminalWake {
        stream: Mutex::new(Some(stream.clone())),
        ready: AtomicBool::new(false),
    });
    let waker = Waker::from(Arc::clone(&witness));
    let mut cx = Context::from_waker(&waker);
    let mut first = Box::pin(stream.next());
    assert!(first.as_mut().poll(&mut cx).is_pending());
    let mut second = Box::pin(stream.next());
    let mut third = Box::pin(stream.next());
    assert!(poll(second.as_mut()).is_pending());
    assert!(poll(third.as_mut()).is_pending());
    release.send(()).unwrap();
    run(task);
    assert!(witness.ready.load(Ordering::SeqCst));
    witness.stream.lock().unwrap().take();
    let AgentEvent::AgentEnd { messages } = run(first).unwrap() else {
        panic!("terminal missing")
    };
    assert!(run(second).is_none());
    assert!(run(third).is_none());
    for observed in [run(result), run(stream.result()), run(stream.result())] {
        assert_eq!(observed.len(), messages.len());
        for (a, b) in observed.iter().zip(&messages) {
            same_entry(a, b);
        }
    }
}
/// A retained test waker inspects result readiness at terminal delivery.
struct TerminalWake {
    /// Released explicitly after the observation.
    stream: Mutex<Option<EventStream<AgentEvent, Vec<AgentMessage>>>>,
    /// Whether result was ready inside the selected reader wake.
    ready: AtomicBool,
}
impl Wake for TerminalWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        let stream = self.stream.lock().unwrap().as_ref().unwrap().clone();
        let mut result = Box::pin(stream.result());
        self.ready
            .store(poll(result.as_mut()).is_ready(), Ordering::SeqCst);
    }
}
/// Compare shared entry allocations rather than only their rendered values.
fn same_entry(a: &AgentMessage, b: &AgentMessage) {
    match (a, b) {
        (AgentMessage::User(a), AgentMessage::User(b)) => assert!(Arc::ptr_eq(a, b)),
        (AgentMessage::Assistant(a), AgentMessage::Assistant(b)) => assert!(Arc::ptr_eq(a, b)),
        (AgentMessage::ToolResult(a), AgentMessage::ToolResult(b)) => assert!(Arc::ptr_eq(a, b)),
        _ => panic!("entry variants differ"),
    }
}
/// Assistant tails fail synchronously without entering caller callbacks.
#[test]
fn maestro_raw_continue_rejects_assistant_before_submission() {
    let context = context();
    *context.messages.write().unwrap() = vec![AgentMessage::Assistant(assistant("old"))];
    let mut config = config();
    config.convert_to_llm = Arc::new(|_| panic!("conversion before admission"));
    let tasks = Arc::new(owned::Tasks::default());
    let error = agent_loop_continue(
        context,
        Arc::new(config),
        responses(vec![]),
        tasks.spawner(),
    )
    .err()
    .unwrap();
    assert_eq!(
        error.message,
        "Cannot continue from message role: assistant"
    );
    assert!(tasks.0.lock().unwrap().is_empty());
}
/// Invocation overrides preserve model, options and executable tool operands.
#[test]
fn maestro_wrapper_overrides_reach_request_and_tool() {
    for (continuation, present, stage) in [false, true].into_iter().flat_map(|c| {
        [false, true]
            .into_iter()
            .flat_map(move |p| ["transform", "model", "tool"].map(|s| (c, p, s)))
    }) {
        check_overrides(continuation, present.then(Cancellation::new), stage);
    }
}
/// Probe one consuming stage with a fresh caller signal.
fn check_overrides(continuation: bool, signal: Option<Cancellation>, stage: &'static str) {
    // A call whose run signal aborted before preparation is not executed and the next turn follows.
    let early_abort = signal.is_some() && stage != "tool";
    let configured = Cancellation::new();
    configured.abort();
    let mut config = config();
    config.options.common.signal = Some(configured);
    config.extra.insert(
        "ordered".into(),
        serde_json::json!(["zulu", "alpha", "zulu"]),
    );
    let model = Arc::clone(&config.model);
    let expected = signal.clone();
    let transformed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    config.transform_context = Some(Arc::new(move |history, actual| {
        if stage == "transform" && !transformed.swap(true, Ordering::SeqCst) {
            check_signal(actual.as_ref(), expected.as_ref());
        }
        ready(Ok(history))
    }));
    let expected = signal.clone();
    let executed = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&executed);
    let context = with_execute(Arc::new(move |_, _, actual, _| {
        if stage == "tool" {
            check_signal(actual.as_ref(), expected.as_ref());
        }
        count.fetch_add(1, Ordering::SeqCst);
        ready(Ok(result()))
    }));
    let expected = signal.clone();
    let requests = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&requests);
    let options = AgentLoopOptions {
        signal,
        stream_fn: Some(Arc::new(move |descriptor, request, options, extra| {
            assert_eq!(descriptor.id, model.read().unwrap().id);
            assert_eq!(request.system_prompt.as_deref(), Some("instructions"));
            assert_eq!(
                extra.extra["ordered"],
                serde_json::json!(["zulu", "alpha", "zulu"])
            );
            let turn = count.fetch_add(1, Ordering::SeqCst);
            if stage == "model" && turn == 0 {
                check_signal(options.common.signal.as_ref(), expected.as_ref());
            }
            let stream = AssistantMessageEventStream::new();
            stream.end(Some(tool_then_stop(turn)));
            ready(Ok(stream))
        })),
    };
    let tasks = Arc::new(owned::Tasks::default());
    let stream = start(continuation, context, config, options, &tasks);
    run(tasks.take());
    assert_eq!(executed.load(Ordering::SeqCst), usize::from(!early_abort));
    assert_eq!(
        requests.load(Ordering::SeqCst),
        if early_abort { 2 } else { 1 }
    );
    let last = drain(&stream).messages.last().unwrap().clone();
    assert_eq!(matches!(last, AgentMessage::ToolResult(_)), !early_abort);
}
/// The first turn calls the tool; later turns end the run.
fn tool_then_stop(turn: usize) -> maestro_models::SharedAssistantMessage {
    if turn == 0 {
        tool_call()
    } else {
        assistant("stopped")
    }
}
/// Distinguish omission, state and shared cancellation identity.
fn check_signal(actual: Option<&Cancellation>, expected: Option<&Cancellation>) {
    match (actual, expected) {
        (None, None) => (),
        (Some(a), Some(b)) => {
            assert!(!a.is_aborted());
            assert!(!b.is_aborted());
            a.abort();
            assert!(b.is_aborted());
        }
        _ => panic!("signal presence differs"),
    }
}
/// Default dispatch and supplied streams select distinct adapters.
#[test]
fn maestro_wrapper_uses_registered_default_stream() {
    use maestro_models::{ApiProvider, register_api_provider, unregister_api_providers};
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    register_api_provider(
        ApiProvider {
            api: "raw-controlled".into(),
            stream: Arc::new(UnexpectedRaw),
            stream_simple: Arc::new(RawDispatch(count)),
        },
        Some("raw-controlled-test".into()),
    );
    for supplied in [false, true] {
        let config = config();
        config.model.write().unwrap().api = "raw-controlled".into();
        let tasks = Arc::new(owned::Tasks::default());
        let stream = start(
            true,
            context(),
            config,
            if supplied {
                responses(vec![assistant("override")])
            } else {
                AgentLoopOptions::default()
            },
            &tasks,
        );
        run(tasks.take());
        assert_eq!(
            text(&drain(&stream).messages[0]),
            if supplied { "override" } else { "registered" }
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    unregister_api_providers("raw-controlled-test");
}
/// Rejections retain their prefix and remain unsettled, with one native diagnostic.
#[test]
fn maestro_raw_failure_reports_without_finishing_stream() {
    if maestro_test_process::selection().is_child() {
        failure_child();
        return;
    }
    let output = maestro_test_process::TestProcess::current(
        "maestro_raw_failure_reports_without_finishing_stream",
    )
    .unwrap()
    .nocapture()
    .case("*")
    .output()
    .unwrap();
    assert!(output.status.success());

    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "transform failure\nconversion failure\nmodel failure\nstop failure\ntransform failure\nconversion failure\nmodel failure\nstop failure\n"
    );
}
/// Exercise every rejection phase inside the same compiled test executable.
fn failure_child() {
    for continuation in [false, true] {
        for phase in ["transform", "conversion", "model", "stop"] {
            failure_case(continuation, phase);
        }
    }
    assert_eq!(
        observe(
            vec![],
            context(),
            config(),
            responses(vec![assistant("later")])
        )
        .messages
        .iter()
        .map(text)
        .collect::<Vec<_>>(),
        ["later"]
    );
}
/// Identical adapters give identical requests, entries and event order.
#[test]
fn maestro_awaited_and_observable_share_adapters() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let first_requests = Arc::new(Mutex::new(Vec::new()));
    let second_requests = Arc::new(Mutex::new(Vec::new()));
    let messages = run(maestro_agent::run_agent_loop(
        vec![user("prompt")],
        echo_context(),
        &config(),
        Arc::new(move |event| {
            observed.lock().unwrap().push(kind(&event));
            ready(Ok(()))
        }),
        request_adapter(&first_requests),
    ))
    .unwrap();
    let recorded = observe(
        vec![user("prompt")],
        echo_context(),
        config(),
        request_adapter(&second_requests),
    );
    assert_eq!(
        *first_requests.lock().unwrap(),
        *second_requests.lock().unwrap()
    );
    assert_eq!(first_requests.lock().unwrap().len(), 2);
    assert_eq!(
        messages
            .iter()
            .map(|m| match m {
                AgentMessage::Assistant(_) => "assistant".to_owned(),
                _ => text(m),
            })
            .collect::<Vec<_>>(),
        recorded
            .messages
            .iter()
            .map(|m| match m {
                AgentMessage::Assistant(_) => "assistant".to_owned(),
                _ => text(m),
            })
            .collect::<Vec<_>>()
    );
    assert_eq!(
        *events.lock().unwrap(),
        recorded.events.iter().map(kind).collect::<Vec<_>>()
    );
    assert_eq!(text(&recorded.messages[2]), "echo operand");
    assert_eq!(text(&messages[3]), "done");
    assert_eq!(text(&recorded.messages[3]), "done");
}
/// Empty and repeated prompts preserve authored order without old history.
#[test]
fn maestro_empty_and_repeated_prompts_keep_order() {
    for inputs in [vec![], vec!["zulu", "alpha", "zulu"]] {
        let context = context();
        let history = Arc::clone(&context.messages);
        let prompts = inputs.iter().map(|s| user(s)).collect();
        let recorded = observe(
            prompts,
            context,
            config(),
            responses(vec![assistant("done")]),
        );
        let mut expected = inputs.clone();
        expected.push("done");
        assert_eq!(
            recorded.messages.iter().map(text).collect::<Vec<_>>(),
            expected
        );
        let users = recorded
            .events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::MessageStart {
                    message: AgentMessage::User(value),
                } => Some(match &value.read().unwrap().content {
                    maestro_models::UserContent::Text(t) => t.clone(),
                    maestro_models::UserContent::Blocks(_) => panic!("text expected"),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(users, inputs);
        assert_eq!(
            history.read().unwrap().iter().map(text).collect::<Vec<_>>(),
            ["earlier"]
        );
    }
}
/// Tool-result tails are admitted and retained history appends without replay.
#[test]
fn maestro_raw_continue_accepts_tool_result_tail() {
    let context = context();
    let history = Arc::clone(&context.messages);
    *history.write().unwrap() = vec![AgentMessage::ToolResult(Arc::new(std::sync::RwLock::new(
        maestro_models::ToolResultMessage {
            tool_call_id: "tail".into(),
            tool_name: "echo".into(),
            content: artifact("tail payload", false).content,
            details: None,
            is_error: false,
            timestamp: 1.0,
        },
    )))];
    let mut config = config();
    let conversions = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&conversions);
    let convert = Arc::clone(&config.convert_to_llm);
    config.convert_to_llm = Arc::new(move |entries| {
        count.fetch_add(1, Ordering::SeqCst);
        assert!(matches!(
            entries.read().unwrap()[0],
            AgentMessage::ToolResult(_)
        ));
        convert(entries)
    });
    let tasks = Arc::new(owned::Tasks::default());
    let options = AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(|_, request, _, _| {
            let maestro_models::Message::ToolResult(value) = &request.messages[0] else {
                panic!("tool result missing from request")
            };
            assert_eq!(value.tool_call_id, "tail");
            assert_eq!(value.tool_name, "echo");
            let maestro_models::UserBlock::Text(value) = &value.content[0] else {
                panic!("tail payload missing")
            };
            assert_eq!(value.text, "tail payload");
            let stream = AssistantMessageEventStream::new();
            stream.end(Some(assistant("done")));
            ready(Ok(stream))
        })),
    };
    let stream = start(true, context, config, options, &tasks);
    assert_eq!(conversions.load(Ordering::SeqCst), 0);
    run(tasks.take());
    assert_eq!(conversions.load(Ordering::SeqCst), 1);
    let recorded = drain(&stream);
    assert_eq!(
        recorded.messages.iter().map(text).collect::<Vec<_>>(),
        ["done"]
    );
    assert_eq!(history.read().unwrap().len(), 2);
    assert!(!recorded.events.iter().any(|e| matches!(
        e,
        AgentEvent::MessageStart {
            message: AgentMessage::ToolResult(_)
        }
    )));
    same_entry(&history.read().unwrap()[1], &recorded.messages[0]);
}
/// Returned failed assistants complete normally rather than rejecting the producer.
#[test]
fn maestro_failed_assistant_still_finishes_raw_stream() {
    for reason in [StopReason::Error, StopReason::Aborted] {
        let response = assistant("failed");
        response.write().unwrap().stop_reason = reason;
        let recorded = observe(
            vec![],
            context(),
            config(),
            responses(vec![Arc::clone(&response)]),
        );
        assert!(
            matches!(recorded.events[recorded.events.len()-2],AgentEvent::TurnEnd { ref tool_results,.. } if tool_results.is_empty())
        );
        assert!(matches!(
            recorded.events.last().unwrap(),
            AgentEvent::AgentEnd { .. }
        ));
        let AgentMessage::Assistant(returned) = &recorded.messages[0] else {
            panic!("failed response lost")
        };
        assert!(Arc::ptr_eq(returned, &response));
    }
}
/// Updates keep the original model and progress payloads in emission order.
#[test]
fn maestro_raw_updates_keep_payloads_and_order() {
    let response = tool_call();
    let partial = Arc::clone(&response);
    let options = AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |_, _, _, _| {
            let stream = AssistantMessageEventStream::new();
            stream.push(maestro_models::AssistantMessageEvent::Start {
                partial: Arc::clone(&partial),
            });
            stream.push(maestro_models::AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "delta".into(),
                partial: Arc::clone(&partial),
            });
            stream.end(Some(Arc::clone(&partial)));
            ready(Ok(stream))
        })),
    };
    let context = with_execute(Arc::new(|_, _, _, update| {
        update.unwrap()(artifact("progress", false));
        ready(Ok(artifact("complete", true)))
    }));
    let recorded = observe(vec![], context, config(), options);
    check_update_payloads(&recorded.events, &response);
    assert_eq!(
        recorded.events.iter().map(kind).collect::<Vec<_>>(),
        [
            "agent_start",
            "turn_start",
            "message_start",
            "message_update",
            "message_end",
            "tool_execution_start",
            "tool_execution_update",
            "tool_execution_end",
            "message_start",
            "message_end",
            "turn_end",
            "agent_end"
        ]
    );
    assert_eq!(text(&recorded.messages[1]), "complete");
}
/// Invoke either public entrypoint with the same caller-owned execution facility.
fn start(
    continuation: bool,
    context: maestro_agent::AgentContext,
    config: maestro_agent::AgentLoopConfig,
    options: AgentLoopOptions,
    tasks: &Arc<owned::Tasks>,
) -> EventStream<AgentEvent, Vec<AgentMessage>> {
    if continuation {
        agent_loop_continue(context, Arc::new(config), options, tasks.spawner()).unwrap()
    } else {
        agent_loop(
            vec![user("prompt")],
            context,
            Arc::new(config),
            options,
            tasks.spawner(),
        )
    }
}

/// Hold the stop hook after turn completion for terminal-boundary observations.
fn held_terminal() -> (
    EventStream<AgentEvent, Vec<AgentMessage>>,
    Arc<owned::Tasks>,
    tokio::sync::oneshot::Sender<()>,
) {
    let (release, gate) = tokio::sync::oneshot::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let mut config = config();
    config.should_stop_after_turn = Some(Arc::new(move |_| {
        let gate = gate.lock().unwrap().take().unwrap();
        Box::pin(async move {
            gate.await.unwrap();
            Ok(true)
        })
    }));
    let tasks = Arc::new(owned::Tasks::default());
    let stream = start(
        false,
        context(),
        config,
        responses(vec![assistant("done")]),
        &tasks,
    );
    (stream, tasks, release)
}

/// Inspect the preserved event prefix and pending observations for one rejection.
fn failure_case(continuation: bool, phase: &str) {
    let (config, options) = failure_adapters(phase);
    let tasks = Arc::new(owned::Tasks::default());
    let stream = start(continuation, context(), config, options, &tasks);
    run(tasks.take());
    let expected = if phase == "stop" {
        if continuation { 5 } else { 7 }
    } else if continuation {
        2
    } else {
        4
    };
    let mut prefix = Vec::new();
    for _ in 0..expected {
        let mut next = Box::pin(stream.next());
        let std::task::Poll::Ready(Some(event)) = poll(next.as_mut()) else {
            panic!("prefix missing")
        };
        prefix.push(kind(&event));
    }
    assert_eq!(&prefix[..2], ["agent_start", "turn_start"]);
    if !continuation {
        assert_eq!(&prefix[2..4], ["message_start", "message_end"]);
    }
    if phase == "stop" {
        assert_eq!(
            &prefix[expected - 3..],
            ["message_start", "message_end", "turn_end"]
        );
    }
    let mut next = Box::pin(stream.next());
    let mut result = Box::pin(stream.result());
    assert!(poll(next.as_mut()).is_pending());
    assert!(poll(result.as_mut()).is_pending());
}

/// Select a source-owned failing phase without bypassing the awaited core.
fn failure_adapters(phase: &str) -> (maestro_agent::AgentLoopConfig, AgentLoopOptions) {
    let mut config = config();
    let mut options = responses(vec![assistant("done")]);
    match phase {
        "transform" => {
            config.transform_context = Some(Arc::new(|_, _| {
                ready(Err(owned::failure("transform failure")))
            }));
        }
        "conversion" => {
            config.convert_to_llm = Arc::new(|_| ready(Err(owned::failure("conversion failure"))));
        }
        "model" => {
            options.stream_fn = Some(Arc::new(|_, _, _, _| {
                ready(Err(owned::failure("model failure")))
            }));
        }
        _ => {
            config.should_stop_after_turn =
                Some(Arc::new(|_| ready(Err(owned::failure("stop failure")))));
        }
    }
    (config, options)
}

/// Assert unchanged update payloads alongside the full trace asserted by the caller.
fn check_update_payloads(events: &[AgentEvent], response: &maestro_models::SharedAssistantMessage) {
    let updates = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::MessageUpdate {
                assistant_message_event:
                    maestro_models::AssistantMessageEvent::TextDelta {
                        delta,
                        partial: observed,
                        ..
                    },
                ..
            } => {
                assert!(Arc::ptr_eq(observed, response));
                Some(delta.as_str())
            }
            AgentEvent::ToolExecutionUpdate {
                partial_result,
                tool_call,
            } => {
                assert_eq!(tool_call.id, "id");
                assert_eq!(
                    partial_result.details,
                    Some(serde_json::json!({"output":"progress"}))
                );
                Some("progress")
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(updates, ["delta", "progress"]);
}

/// The same executable adapter is used by awaited and observable callers.
fn echo_context() -> maestro_agent::AgentContext {
    with_execute(Arc::new(|id, args, _, _| {
        assert_eq!(id, "id");
        assert!(args.as_object().unwrap().is_empty());
        ready(Ok(artifact("echo operand", false)))
    }))
}
/// Record model requests using the same controlled response constructor.
fn request_adapter(requests: &Arc<Mutex<Vec<serde_json::Value>>>) -> AgentLoopOptions {
    let requests = Arc::clone(requests);
    let source = responses(vec![tool_call(), assistant("done")])
        .stream_fn
        .unwrap();
    AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |model, request, options, extra| {
            let mut value = serde_json::json!({"instructions":request.system_prompt,"messages":request.messages,"tools":request.tools});
            for message in value["messages"].as_array_mut().unwrap() {
                if message["role"] == "toolResult" {
                    assert!(message["timestamp"].as_f64().unwrap().is_finite());
                    message.as_object_mut().unwrap().shift_remove("timestamp");
                }
            }
            requests.lock().unwrap().push(value);
            source(model, request, options, extra)
        })),
    }
}

/// Retained invocation owner for the controlled provider.
struct RawDispatch(Arc<AtomicUsize>);
impl<E: maestro_models::ProviderExecution>
    maestro_models::Invoke<
        E,
        maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    > for RawDispatch
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(
        &self,
        call: maestro_models::ProviderCall<E, maestro_models::SimpleStreamOptions<E>>,
    ) -> Self::Return {
        let maestro_models::ProviderCall {
            model,
            context: request,
            ..
        } = call;
        let count = &self.0;

        assert_eq!(model.id, "fixture");
        assert_eq!(request.messages.len(), 1);
        count.fetch_add(1, Ordering::SeqCst);
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(assistant("registered")));
        Ok(stream)
    }
}

/// Retained invocation owner for the controlled provider.
struct UnexpectedRaw;
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for UnexpectedRaw
{
    type Return =
        Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, _call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        panic!("raw dispatch selected")
    }
}
