//! Turn controls through awaited conversation operations.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{AgentEvent, AgentMessage, run_agent_loop};
use std::sync::{Arc, Mutex};
use support::*;

/// Stop sees the completed turn after its awaited event and skips subsequent polls.
#[test]
fn stop_hook_observes_completed_turn_before_queue_polls() {
    run(async {
        stop_case().await;
        stop_gate_case().await;
    });
}
/// Exercise completed event and callback phases.
async fn stop_case() {
    let trace = Arc::new(Mutex::new(Vec::new()));
    let mut config = config();
    let captured = Arc::clone(&trace);
    config.get_steering_messages = Some(Arc::new(move || {
        captured.lock().unwrap().push("steering");
        ready(Ok(vec![]))
    }));
    let captured = Arc::clone(&trace);
    config.should_stop_after_turn = Some(Arc::new(move |c| {
        captured.lock().unwrap().push("stop");
        assert_eq!(c.new_messages.len(), 2);
        assert!(c.tool_results.is_empty());
        assert_eq!(c.context.messages.read().unwrap().len(), 3);
        Box::pin(async { Ok(true) })
    }));
    config.get_follow_up_messages = Some(Arc::new(|| panic!("stopped before follow-up")));
    let captured = Arc::clone(&trace);
    let returned = run_agent_loop(
        vec![user("go")],
        context(),
        &config,
        Arc::new(move |e| {
            let captured = Arc::clone(&captured);
            Box::pin(async move {
                if matches!(e, AgentEvent::TurnEnd { .. }) {
                    captured.lock().unwrap().push("turn");
                }
                Ok(())
            })
        }),
        responses(vec![assistant("done")]),
    )
    .await
    .unwrap();
    assert_eq!(returned.len(), 2);
    assert_eq!(*trace.lock().unwrap(), ["steering", "turn", "stop"]);
}

/// Compare complete public observations for initial steering follows entry events.
#[test]
fn initial_steering_follows_entry_events() {
    run(async {
        entry_gate_case(false).await;
        entry_gate_case(true).await;
        support::corpus::replay_controls(&["initial-steering", "absent-control-callbacks"]).await;
    });
}

/// Compare complete public observations for steering arrives only after whole tool batch.
#[test]
fn steering_arrives_only_after_whole_tool_batch() {
    run(async {
        batch_steering_case(maestro_agent::ToolExecutionMode::Sequential).await;
        batch_steering_case(maestro_agent::ToolExecutionMode::Parallel).await;
        support::corpus::replay_controls(&[
            "post-batch-steering",
            "termination-does-not-suppress-steering",
        ])
        .await;
    });
}

/// Compare complete public observations for followups wait for tools and steering.
#[test]
fn followups_wait_for_tools_and_steering() {
    run(support::corpus::replay_controls(&[
        "follow-up",
        "steering-before-follow",
        "tool-continuation-before-follow",
        "termination-does-not-suppress-follow",
        "repeated-followups",
    ]));
}

/// Compare complete public observations for stop context distinguishes prompt and continuation.
#[test]
fn stop_context_distinguishes_prompt_and_continuation() {
    run(support::corpus::replay_controls(&[
        "stop-false-0",
        "stop-false-2",
        "stop-true-0",
        "stop-true-2",
        "false-stop-continues",
    ]));
}

/// Compare complete public observations for failed assistants skip post turn controls.
#[test]
fn failed_assistants_skip_post_turn_controls() {
    run(support::corpus::replay_controls(&[
        "assistant-error",
        "assistant-aborted",
        "assistant-length",
    ]));
}

/// Compare complete public observations for control failures propagate without synthetic completion.
#[test]
fn control_failures_propagate_without_synthetic_completion() {
    run(async {
        injection_failure_case().await;
        support::corpus::replay_controls(&[
            "failure-initial-steering",
            "failure-post-steering",
            "failure-follow",
            "failure-stop",
            "failure-turn-end",
        ])
        .await;
    });
}

/// Ordered queue handles append only after each awaited observation pair.
#[test]
fn queue_injection_keeps_order_aliases_and_event_barriers() {
    run(queue_case());
}
/// Preserve repeated shared entries through steering and follow-up injection.
async fn queue_case() {
    use futures_util::FutureExt;
    use maestro_agent::{AgentMessage, run_agent_loop_continue};
    let zulu = AgentMessage::Custom(Arc::new(std::sync::RwLock::new(QueueNotice {
        text: "zulu".into(),
    })));
    let alpha = queue_user("alpha");
    let queued = [zulu.clone(), alpha.clone(), zulu.clone()];
    let context = maestro_agent::AgentContext {
        system_prompt: "instructions".into(),
        messages: Arc::new(std::sync::RwLock::new(vec![queue_user("earlier")])),
        tools: None,
    };
    let history = Arc::clone(&context.messages);
    let mut config = queue_config();
    let batches = Arc::new(Mutex::new(std::collections::VecDeque::from([
        queued.to_vec(),
        vec![],
    ])));
    config.get_steering_messages = Some(Arc::new(move || {
        ready(Ok(batches.lock().unwrap().pop_front().unwrap_or_default()))
    }));
    let follow = Arc::new(Mutex::new(Some(vec![alpha.clone()])));
    config.get_follow_up_messages = Some(Arc::new(move || {
        ready(Ok(follow.lock().unwrap().take().unwrap_or_default()))
    }));
    let (release, wait) = tokio::sync::oneshot::channel();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let sink = queue_sink(Arc::clone(&history), wait);
    let mut operation = Box::pin(run_agent_loop_continue(
        context,
        &config,
        sink,
        responses(vec![assistant("first"), assistant("second")]),
    ));
    let pending = operation.as_mut().now_or_never().is_none();
    let before_append = history.read().unwrap().len();
    release.send(()).unwrap();
    let returned = operation.await.unwrap();
    assert!(pending, "queue injection must await message end");
    assert_eq!(before_append, 1);
    assert_eq!(returned.len(), 6);
    assert_queue_aliases(&returned, &zulu, &alpha);
    let AgentMessage::Custom(notice) = &zulu else {
        panic!("notice");
    };
    let weak = Arc::downgrade(notice);
    drop((config, history, zulu, alpha, queued));
    assert!(weak.upgrade().is_some());
    drop(returned);
    assert!(weak.upgrade().is_none());
}

/// Gate the first injected entry before it can enter retained history.
fn queue_sink<C: maestro_agent::CustomAgentMessages + Send + Sync + 'static>(
    captured: Arc<std::sync::RwLock<Vec<AgentMessage<C>>>>,
    wait: Arc<Mutex<Option<tokio::sync::oneshot::Receiver<()>>>>,
) -> maestro_agent::AgentEventSink<C> {
    Arc::new(move |e| {
        if let AgentEvent::MessageEnd {
            message: AgentMessage::User(_) | AgentMessage::Custom(_),
        } = e
        {
            let wait = wait.lock().unwrap().take();
            if let Some(wait) = wait {
                assert_eq!(captured.read().unwrap().len(), 1);
                return wait_for_end(wait);
            }
        }
        ready(Ok(()))
    })
}

/// Await the controlled message-end completion witness.
fn wait_for_end(
    wait: tokio::sync::oneshot::Receiver<()>,
) -> maestro_models::BoxFuture<Result<(), maestro_models::DiagnosticErrorInfo>> {
    Box::pin(async move {
        wait.await.unwrap();
        Ok(())
    })
}

/// Completed `TurnEnd` and async stop settlement each gate subsequent controls.
async fn stop_gate_case() {
    use futures_util::FutureExt;
    let trace = Arc::new(Mutex::new(Vec::new()));
    let (turn_release, turn_wait) = tokio::sync::oneshot::channel();
    let (stop_release, stop_wait) = tokio::sync::oneshot::channel();
    let turn_wait = Arc::new(Mutex::new(Some(turn_wait)));
    let mut config = config();
    config.should_stop_after_turn = Some(gated_stop(Arc::clone(&trace), stop_wait));
    config.get_follow_up_messages = Some(Arc::new(|| panic!("stop prevents follow-up poll")));
    let context = with_execute(Arc::new(|_, _, _, _| ready(Ok(result()))));
    let captured = Arc::clone(&trace);
    let sink = Arc::new(move |event| {
        if matches!(event, AgentEvent::TurnEnd { .. }) {
            let wait = turn_wait.lock().unwrap().take().unwrap();
            let captured = Arc::clone(&captured);
            return Box::pin(async move {
                wait.await.unwrap();
                captured.lock().unwrap().push("turn settled");
                Ok(())
            }) as maestro_models::BoxFuture<_>;
        }
        ready(Ok(()))
    });
    let mut operation = Box::pin(run_agent_loop(
        vec![user("prompt")],
        context,
        &config,
        sink,
        responses(vec![tool_call()]),
    ));
    let first = operation.as_mut().now_or_never();
    let before_turn = trace.lock().unwrap().clone();
    let _ = turn_release.send(());
    let second = if first.is_none() {
        operation.as_mut().now_or_never()
    } else {
        None
    };
    let before_stop = trace.lock().unwrap().clone();
    let _ = stop_release.send(());
    let returned = match first.or(second) {
        Some(value) => value,
        None => operation.await,
    }
    .unwrap();
    assert!(before_turn.is_empty(), "stop waits for TurnEnd settlement");
    assert_eq!(before_stop, ["turn settled", "stop entered"]);
    assert_eq!(
        *trace.lock().unwrap(),
        ["turn settled", "stop entered", "stop settled"]
    );
    assert_eq!(returned.len(), 3);
}
/// Borrow completed tool records across an awaited stop decision.
fn gated_stop(
    trace: Arc<Mutex<Vec<&'static str>>>,
    wait: tokio::sync::oneshot::Receiver<()>,
) -> maestro_agent::ShouldStopAfterTurn<maestro_agent::NoCustomMessages> {
    let wait = Mutex::new(Some(wait));
    Arc::new(move |c| {
        assert_eq!(c.tool_results.len(), 1);
        assert_eq!(c.new_messages.len(), 3);
        assert_eq!(c.context.messages.read().unwrap().len(), 4);
        trace.lock().unwrap().push("stop entered");
        let wait = wait.lock().unwrap().take().unwrap();
        let trace = Arc::clone(&trace);
        Box::pin(async move {
            wait.await.unwrap();
            trace.lock().unwrap().push("stop settled");
            Ok(true)
        })
    })
}

/// Caller notice that deliberately has no Clone implementation.
struct QueueNotice {
    /// Text observed through the retained handle.
    text: String,
}
impl maestro_agent::CustomAgentMessages for QueueNotice {
    fn role(&self) -> &'static str {
        "notice"
    }
}
/// Use the existing user fixture in a caller-defined message domain.
fn queue_user(text: &str) -> AgentMessage<QueueNotice> {
    let AgentMessage::User(user) = user(text) else {
        unreachable!()
    };
    AgentMessage::User(user)
}
/// Convert heterogeneous queue entries only at the model request boundary.
fn queue_config() -> maestro_agent::AgentLoopConfig<QueueNotice> {
    let base = config();
    maestro_agent::AgentLoopConfig {
        thinking_budgets: None,
        model: base.model,
        options: base.options,
        extra: base.extra,
        objects: base.objects,
        convert_to_llm: Arc::new(|messages| {
            ready(Ok(messages
                .read()
                .unwrap()
                .iter()
                .map(queue_convert)
                .collect()))
        }),
        transform_context: None,
        get_api_key: None,
        before_tool_call: None,
        after_tool_call: None,
        should_stop_after_turn: None,
        prepare_next_turn: None,
        get_steering_messages: None,
        get_follow_up_messages: None,
        tool_execution: base.tool_execution,
    }
}
/// Convert a shared record without copying its caller-defined payload.
fn queue_convert(message: &AgentMessage<QueueNotice>) -> maestro_models::Message {
    use maestro_models::Message;
    match message {
        AgentMessage::User(value) => Message::User(value.read().unwrap().clone()),
        AgentMessage::Assistant(value) => Message::Assistant(value.read().unwrap().clone()),
        AgentMessage::ToolResult(value) => Message::ToolResult(value.read().unwrap().clone()),
        AgentMessage::Custom(value) => Message::User(maestro_models::UserMessage {
            content: maestro_models::UserContent::Text(value.read().unwrap().text.clone()),
            timestamp: 1.0,
        }),
    }
}

/// Observe shared identity and edits through heterogeneous retained queue entries.
fn assert_queue_aliases(
    returned: &[AgentMessage<QueueNotice>],
    zulu: &AgentMessage<QueueNotice>,
    alpha: &AgentMessage<QueueNotice>,
) {
    for (index, original) in [(0, zulu), (1, alpha), (2, zulu), (4, alpha)] {
        match (&returned[index], original) {
            (AgentMessage::User(returned), AgentMessage::User(original)) => {
                assert!(Arc::ptr_eq(returned, original));
            }
            (AgentMessage::Custom(returned), AgentMessage::Custom(original)) => {
                assert!(Arc::ptr_eq(returned, original));
            }
            _ => panic!("queued entry kind"),
        }
    }
    let AgentMessage::Custom(zulu) = zulu else {
        panic!("notice");
    };
    zulu.write().unwrap().text = "edited through alias".into();
    let AgentMessage::Custom(repeated) = &returned[2] else {
        panic!("notice");
    };
    assert_eq!(repeated.read().unwrap().text, "edited through alias");
}

/// Queue polls wait for the last finalizer in either execution mode.
async fn batch_steering_case(mode: maestro_agent::ToolExecutionMode) {
    use futures_util::FutureExt;
    let context = with_execute(Arc::new(|_, _, _, _| ready(Ok(result()))));
    let mut config = config();
    config.tool_execution = Some(mode);
    let polls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = Arc::clone(&polls);
    config.get_steering_messages = Some(Arc::new(move || {
        let index = observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        ready(Ok(if index == 1 {
            vec![user("steered")]
        } else {
            vec![]
        }))
    }));
    let calls = tool_call();
    let mut second = calls.read().unwrap().content[0].clone();
    if let maestro_models::AssistantContent::ToolCall(call) = &mut second {
        call.id = "second".into();
    }
    calls.write().unwrap().content.push(second);
    let (release, wait) = tokio::sync::oneshot::channel();
    let sink = batch_end_gate(wait);
    let mut operation = Box::pin(run_agent_loop(
        vec![],
        context,
        &config,
        sink,
        responses(vec![calls, assistant("after steering")]),
    ));
    let first = operation.as_mut().now_or_never();
    let prior_polls = polls.load(std::sync::atomic::Ordering::SeqCst);
    let pending = first.is_none();
    let _ = release.send(());
    let returned = match first {
        Some(value) => value,
        None => operation.await,
    }
    .unwrap();
    assert!(pending, "last finalizer must gate post-turn steering");
    assert_eq!(
        prior_polls, 1,
        "only entry steering may run before the batch settles"
    );
    assert_eq!(returned.len(), 5);
    assert!(
        matches!(&returned[3], AgentMessage::User(u) if matches!(&u.read().unwrap().content, maestro_models::UserContent::Text(t) if t == "steered"))
    );
}
/// Await the second call's final observation without a timer or detached helper.
fn batch_end_gate(wait: tokio::sync::oneshot::Receiver<()>) -> maestro_agent::AgentEventSink {
    let wait = Mutex::new(Some(wait));
    Arc::new(move |event| {
        if let AgentEvent::ToolExecutionEnd { tool_call, .. } = event
            && tool_call.id == "second"
        {
            return wait_for_end(wait.lock().unwrap().take().unwrap());
        }
        ready(Ok(()))
    })
}

/// A queued message observation failure keeps earlier effects without appending it.
async fn injection_failure_case() {
    let context = context();
    let history = Arc::clone(&context.messages);
    let mut config = config();
    config.get_steering_messages =
        Some(Arc::new(|| ready(Ok(vec![user("queued"), user("later")]))));
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let sink = Arc::new(move |event| {
        let (label, failure) = match event {
            AgentEvent::AgentStart => ("agent", false),
            AgentEvent::TurnStart => ("turn", false),
            AgentEvent::MessageStart { .. } => ("start", false),
            AgentEvent::MessageEnd { .. } => ("end", true),
            _ => panic!("observation failure must not synthesize later events"),
        };
        observed.lock().unwrap().push(label);
        ready(if failure {
            Err(control_error("queued message failed"))
        } else {
            Ok(())
        })
    });
    let error = maestro_agent::run_agent_loop_continue(context, &config, sink, responses(vec![]))
        .await
        .err()
        .unwrap();
    assert_eq!(error.message, "queued message failed");
    assert_eq!(*events.lock().unwrap(), ["agent", "turn", "start", "end"]);
    assert_eq!(history.read().unwrap().len(), 1);
}
/// Controlled callback diagnostic with no credential-bearing payload.
fn control_error(message: &str) -> maestro_models::DiagnosticErrorInfo {
    maestro_models::DiagnosticErrorInfo::new(None, message)
}

/// Entry steering cannot run before the final entry observation settles.
async fn entry_gate_case(continuation: bool) {
    use futures_util::FutureExt;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    let settled = Arc::new(AtomicBool::new(false));
    let polls = Arc::new(AtomicUsize::new(0));
    let captured = Arc::clone(&settled);
    let observed = Arc::clone(&polls);
    let mut config = config();
    config.get_steering_messages = Some(Arc::new(move || {
        assert!(
            captured.load(Ordering::SeqCst),
            "entry steering waits for observation settlement"
        );
        observed.fetch_add(1, Ordering::SeqCst);
        ready(Ok(vec![]))
    }));
    let (release, wait) = tokio::sync::oneshot::channel();
    let sink = entry_gate_sink(continuation, wait, Arc::clone(&settled));
    let mut operation = Box::pin(async {
        if continuation {
            maestro_agent::run_agent_loop_continue(
                context(),
                &config,
                sink,
                responses(vec![assistant("done")]),
            )
            .await
        } else {
            run_agent_loop(
                vec![user("prompt")],
                context(),
                &config,
                sink,
                responses(vec![assistant("done")]),
            )
            .await
        }
    });
    let first = operation.as_mut().now_or_never();
    let prior_polls = polls.load(Ordering::SeqCst);
    let pending = first.is_none();
    let _ = release.send(());
    let returned = match first {
        Some(value) => value,
        None => operation.await,
    }
    .unwrap();
    assert!(pending, "entry observation must gate request and steering");
    assert_eq!(prior_polls, 0);
    assert_eq!(returned.len(), if continuation { 1 } else { 2 });
}
/// Hold the prompt end or continuation turn start at the native observation port.
fn entry_gate_sink(
    continuation: bool,
    wait: tokio::sync::oneshot::Receiver<()>,
    settled: Arc<std::sync::atomic::AtomicBool>,
) -> maestro_agent::AgentEventSink {
    let wait = Mutex::new(Some(wait));
    Arc::new(move |event| {
        let gate = if continuation {
            matches!(event, AgentEvent::TurnStart)
        } else {
            matches!(
                event,
                AgentEvent::MessageEnd {
                    message: AgentMessage::User(_)
                }
            )
        };
        if gate {
            let wait = wait.lock().unwrap().take().unwrap();
            let settled = Arc::clone(&settled);
            return Box::pin(async move {
                wait.await.unwrap();
                settled.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }) as maestro_models::BoxFuture<_>;
        }
        ready(Ok(()))
    })
}
