//! Awaited conversation operations through the public loop.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod support;
use maestro_agent::{AgentEvent, AgentMessage, run_agent_loop};
use std::sync::{Arc, Mutex};
use support::*;

/// Prompting retains entries but leaves the supplied outer history unchanged.
#[test]
fn prompt_returns_new_messages_and_keeps_input_history() {
    run(support::corpus::replay(&[
        "prompt",
        "empty-prompt",
        "batched-prompts",
    ]));
    run(prompt_returns_new_messages_and_keeps_input_history_case());
}
/// Execute the controlled public scenario.
async fn prompt_returns_new_messages_and_keeps_input_history_case() {
    let context = context();
    let history = Arc::clone(&context.messages);
    let prompt = user("new");
    let events = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&events);
    let sink = Arc::new(move |event| {
        observed.lock().unwrap().push(kind(&event));
        Box::pin(async { Ok(()) }) as maestro_models::BoxFuture<_>
    });
    let returned = run_agent_loop(
        vec![prompt.clone()],
        context,
        &config(),
        sink,
        responses(vec![assistant("answer")]),
    )
    .await
    .unwrap();
    assert_eq!(history.read().unwrap().len(), 1);
    assert_eq!(returned.len(), 2);
    let (AgentMessage::User(original), AgentMessage::User(retained)) = (&prompt, &returned[0])
    else {
        panic!("expected retained user");
    };
    assert!(Arc::ptr_eq(original, retained));
    assert_eq!(
        *events.lock().unwrap(),
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
}

/// Identify lifecycle variants without hiding their ordering.
fn kind(event: &AgentEvent) -> &'static str {
    match event {
        AgentEvent::AgentStart => "agent_start",
        AgentEvent::AgentEnd { .. } => "agent_end",
        AgentEvent::TurnStart => "turn_start",
        AgentEvent::TurnEnd { .. } => "turn_end",
        AgentEvent::MessageStart { .. } => "message_start",
        AgentEvent::MessageEnd { .. } => "message_end",
        AgentEvent::MessageUpdate { .. } => "message_update",
        AgentEvent::ToolExecutionStart { .. } => "tool_execution_start",
        AgentEvent::ToolExecutionUpdate { .. } => "tool_execution_update",
        AgentEvent::ToolExecutionEnd { .. } => "tool_execution_end",
    }
}

/// Continuation appends through the original handle without replaying its tail.
#[test]
fn continuation_retains_history_without_replaying_tail() {
    run(support::corpus::replay(&[
        "continue",
        "continue-tool-result",
    ]));
    run(continuation_retains_history_without_replaying_tail_case());
}
/// Execute the controlled public scenario.
async fn continuation_retains_history_without_replaying_tail_case() {
    let context = context();
    let history = Arc::clone(&context.messages);
    let returned = maestro_agent::run_agent_loop_continue(
        context,
        &config(),
        sink(),
        responses(vec![assistant("continued")]),
    )
    .await
    .unwrap();
    assert_eq!(history.read().unwrap().len(), 2);
    assert_eq!(returned.len(), 1);
    let (AgentMessage::Assistant(stored), AgentMessage::Assistant(returned)) =
        (&history.read().unwrap()[1], &returned[0])
    else {
        panic!("expected assistant");
    };
    assert!(Arc::ptr_eq(stored, returned));
}

/// Invalid continuation tails are rejected before any callback.
#[test]
fn continuation_rejects_invalid_tails_before_events() {
    run(support::corpus::replay(&[
        "continue-empty",
        "continue-assistant",
    ]));
    run(continuation_rejects_invalid_tails_before_events_case());
}
/// Execute the controlled public scenario.
async fn continuation_rejects_invalid_tails_before_events_case() {
    for (tail, expected) in [
        (None, "Cannot continue: no messages in context"),
        (
            Some(AgentMessage::Assistant(assistant("old"))),
            "Cannot continue from message role: assistant",
        ),
    ] {
        let context = context();
        *context.messages.write().unwrap() = tail.into_iter().collect();
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&count);
        let emit = Arc::new(move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async { Ok(()) }) as maestro_models::BoxFuture<_>
        });
        let error =
            maestro_agent::run_agent_loop_continue(context, &config(), emit, responses(vec![]))
                .await
                .err()
                .expect("guard failure");
        assert_eq!(error.message, expected);
        assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
}

/// Replacement history is transformed before model conversion without replacing raw history.
#[test]
fn transformation_precedes_conversion() {
    run(support::corpus::replay(&[
        "transform-prune",
        "transform-inplace",
    ]));
    run(transformation_precedes_conversion_case());
}
/// Execute the controlled public scenario.
async fn transformation_precedes_conversion_case() {
    let context = context();
    let history = Arc::clone(&context.messages);
    let mut config = config();
    config.transform_context = Some(Arc::new(|_, _| {
        Box::pin(async { Ok(Arc::new(std::sync::RwLock::new(vec![user("transformed")]))) })
    }));
    config.convert_to_llm = Arc::new(|entries| {
        let entries = entries.read().unwrap();
        let AgentMessage::User(entry) = &entries[0] else {
            panic!("expected user");
        };
        assert_eq!(
            entry.read().unwrap().content,
            maestro_models::UserContent::Text("transformed".into())
        );
        Box::pin(async { Ok(Vec::new()) })
    });
    maestro_agent::run_agent_loop_continue(
        context,
        &config,
        sink(),
        responses(vec![assistant("answer")]),
    )
    .await
    .unwrap();
    let entries = history.read().unwrap();
    let AgentMessage::User(entry) = &entries[0] else {
        panic!("expected raw user");
    };
    assert_eq!(
        entry.read().unwrap().content,
        maestro_models::UserContent::Text("earlier".into())
    );
}

/// Key refresh uses empty fallback without trimming or discarding open settings.
#[test]
fn request_options_refresh_keys_and_keep_open_fields() {
    run(support::corpus::replay(&[
        "key-missing",
        "key-empty",
        "key-whitespace",
        "key-fresh",
        "unconfigured-key",
        "empty-key-without-fallback",
        "rich-options",
        "key-refresh",
    ]));
    run(request_options_refresh_keys_and_keep_open_fields_case());
}
/// Execute the controlled public scenario.
async fn request_options_refresh_keys_and_keep_open_fields_case() {
    for (resolved, expected) in [
        (None, "configured"),
        (Some(""), "configured"),
        (Some("fresh"), "fresh"),
        (Some("  "), "  "),
    ] {
        let mut config = config();
        config.options.common.api_key = Some("configured".into());
        config.options.common.temperature = Some(0.75);
        config
            .extra
            .insert("custom".into(), serde_json::json!([false, 8]));
        config.get_api_key = Some(Arc::new(move |_| {
            Box::pin(async move { Ok(resolved.map(str::to_owned)) })
        }));
        let stream_fn = Arc::new(
            move |_,
                  _,
                  options: maestro_models::SimpleStreamOptions,
                  open: maestro_models::ProviderStreamOptions| {
                assert!(
                    options.common.api_key.as_deref() == Some(expected),
                    "credential resolution mismatch"
                );
                assert!(
                    open.common.api_key.as_deref() == Some(expected),
                    "open credential mismatch"
                );
                assert_eq!(options.common.temperature, Some(0.75));
                assert_eq!(open.extra["custom"], serde_json::json!([false, 8]));
                Box::pin(async {
                    let stream = maestro_models::AssistantMessageEventStream::new();
                    stream.end(Some(assistant("answer")));
                    Ok(stream)
                }) as maestro_models::BoxFuture<_>
            },
        );
        maestro_agent::run_agent_loop_continue(
            context(),
            &config,
            sink(),
            maestro_agent::AgentLoopOptions {
                signal: None,
                stream_fn: Some(stream_fn),
            },
        )
        .await
        .unwrap();
    }
}

/// Model updates retain the live partial while event snapshots remain independent.
#[test]
fn assistant_event_updates_preserve_history_and_payloads() {
    run(support::corpus::replay(&[
        "stream-updates",
        "stream-updates-before-start",
    ]));
    run(assistant_event_updates_preserve_history_and_payloads_case());
}
/// Execute the controlled public scenario.
async fn assistant_event_updates_preserve_history_and_payloads_case() {
    use maestro_models::AssistantMessageEvent;
    let context = context();
    let history = Arc::clone(&context.messages);
    let partial = assistant("partial");
    let live = Arc::clone(&partial);
    let snapshots = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&snapshots);
    let emit = Arc::new(move |event| {
        match event {
            AgentEvent::MessageStart {
                message: AgentMessage::Assistant(message),
            } => observed.lock().unwrap().push(message),
            AgentEvent::MessageUpdate {
                message,
                assistant_message_event: AssistantMessageEvent::TextDelta { partial, .. },
            } => {
                let AgentMessage::Assistant(stored) =
                    history.read().unwrap().last().unwrap().clone()
                else {
                    panic!("assistant history");
                };
                assert!(Arc::ptr_eq(&stored, &partial));
                observed.lock().unwrap().push(message);
            }
            _ => (),
        }
        Box::pin(async { Ok(()) }) as maestro_models::BoxFuture<_>
    });
    let options = snapshot_stream_options(live);
    maestro_agent::run_agent_loop_continue(context, &config(), emit, options)
        .await
        .unwrap();
    let snapshots = snapshots.lock().unwrap();
    assert_eq!(snapshots.len(), 2);
    for snapshot in snapshots.iter() {
        assert!(!Arc::ptr_eq(snapshot, &partial));
    }
    partial.write().unwrap().content.clear();
    for snapshot in snapshots.iter() {
        assert_eq!(snapshot.read().unwrap().content.len(), 1);
    }
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn custom_tails_are_converted_only_at_request() {
    run(support::corpus::replay(&[
        "custom-prompt",
        "continue-custom",
    ]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn request_failures_return_without_synthetic_end() {
    run(support::corpus::replay(&[
        "transform-error",
        "convert-error",
        "stream-error",
        "key-error",
    ]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn invocation_signal_replaces_config_signal() {
    run(shared_signal_case());
    run(support::corpus::replay(&["abort-signal", "absent-signal"]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn stream_result_is_authoritative_with_or_without_events() {
    run(pending_result_case(false));
    run(pending_result_case(true));
    run(support::corpus::replay(&[
        "stream-start-eof",
        "stream-start-done",
        "stream-eof",
    ]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn failed_assistants_bypass_tool_execution() {
    run(support::corpus::replay(&[
        "stop-length",
        "stop-error",
        "stop-aborted",
        "failed-assistant-with-tools-error",
        "failed-assistant-with-tools-aborted",
    ]));
}

/// Replay the complete controlled source observations for this branch.
#[test]
fn every_event_is_awaited() {
    for target in [
        "agent_start",
        "turn_start",
        "message_start",
        "message_update",
        "message_end",
        "tool_execution_start",
        "tool_execution_update",
        "tool_execution_end",
        "turn_end",
        "agent_end",
    ] {
        run(event_barrier_case(target));
    }
    run(support::corpus::replay(&[
        "sink-agent_start",
        "sink-turn_start",
        "sink-message_start",
        "sink-message_update",
        "sink-message_end",
        "sink-tool_execution_start",
        "sink-tool_execution_end",
        "sink-turn_end",
        "sink-agent_end",
    ]));
}

/// Registered simple dispatch receives the supplied descriptor and conversation.
#[test]
fn default_stream_uses_registered_model_adapter() {
    use maestro_models::{
        ApiProvider, AssistantMessageEventStream, register_api_provider, unregister_api_providers,
    };
    let observed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let called = Arc::clone(&observed);
    register_api_provider(
        ApiProvider {
            api: "loop-controlled".into(),
            stream: Arc::new(|_, _, _| panic!("raw dispatch is not selected")),
            stream_simple: Arc::new(move |model, context, options| {
                assert_eq!(model.id, "fixture");
                assert_eq!(context.messages.len(), 1);
                assert_eq!(options.unwrap().common.temperature, Some(0.5));
                called.store(true, std::sync::atomic::Ordering::SeqCst);
                let stream = AssistantMessageEventStream::new();
                stream.end(Some(assistant("registered")));
                Ok(stream)
            }),
        },
        Some("loop-controlled-test".into()),
    );
    let mut config = config();
    config.model.api = "loop-controlled".into();
    config.options.common.temperature = Some(0.5);
    let result = run(maestro_agent::run_agent_loop_continue(
        context(),
        &config,
        sink(),
        maestro_agent::AgentLoopOptions::default(),
    ));
    unregister_api_providers("loop-controlled-test");
    assert!(result.is_ok());
    assert!(observed.load(std::sync::atomic::Ordering::SeqCst));
}

/// Every reached caller callback may reacquire retained history and inventory locks.
#[test]
fn callbacks_run_without_history_or_tool_locks() {
    let context = with_execute(Arc::new(|_, _, _, _| ready(Ok(result()))));
    let tools = context.tools.as_ref().unwrap().clone();
    let tool = tools.read().unwrap()[0].clone();
    let weak = Arc::downgrade(&tool);
    tool.write().unwrap().prepare_arguments = Some(Arc::new(move |args| {
        assert!(weak.upgrade().unwrap().try_write().is_ok());
        Ok(args)
    }));
    let weak = Arc::downgrade(&tool);
    tool.write().unwrap().execute = Arc::new(move |_, _, _, _| {
        assert!(weak.upgrade().unwrap().try_write().is_ok());
        ready(Ok(result()))
    });
    let mut config = config();
    let inventory = Arc::clone(&tools);
    config.transform_context = Some(Arc::new(move |history, _| {
        assert!(history.try_write().is_ok());
        assert!(inventory.try_write().is_ok());
        ready(Ok(history))
    }));
    config.convert_to_llm = Arc::new(|history| {
        assert!(history.try_write().is_ok());
        ready(Ok(Vec::new()))
    });
    let options = maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |_, _, _, _| {
            assert!(tools.try_write().is_ok());
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(tool_call()));
            ready(Ok(stream))
        })),
    };
    assert!(
        run(maestro_agent::run_agent_loop_continue(
            context,
            &config,
            sink(),
            options
        ))
        .is_ok()
    );
}

/// Clearing a retained progress observer releases its captured observation state.
#[test]
fn retained_callbacks_release_after_explicit_teardown() {
    let saved = Arc::new(Mutex::new(None));
    let observer = Arc::clone(&saved);
    let context = with_execute(Arc::new(move |_, _, _, update| {
        *observer.lock().unwrap() = update;
        ready(Ok(result()))
    }));
    let resource = Arc::new(());
    let weak = Arc::downgrade(&resource);
    let emit = Arc::new(move |_| {
        assert_eq!(Arc::strong_count(&resource), 1);
        ready(Ok(()))
    });
    assert!(
        run(maestro_agent::run_agent_loop_continue(
            context,
            &config(),
            emit,
            responses(vec![tool_call()])
        ))
        .is_ok()
    );
    assert!(weak.upgrade().is_some());
    let old = saved.lock().unwrap().take();
    drop(old);
    assert!(weak.upgrade().is_none());
}

/// EOF remains pending until the same stream publishes its result.
async fn pending_result_case(started: bool) {
    use futures_util::FutureExt;
    let stream = maestro_models::AssistantMessageEventStream::new();
    if started {
        stream.push(maestro_models::AssistantMessageEvent::Start {
            partial: assistant("partial"),
        });
    }
    stream.end(None);
    let observed = stream.clone();
    let options = maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |_, _, _, _| ready(Ok(observed.clone())))),
    };
    let config = config();
    let mut context = context();
    let history = Arc::clone(&context.messages);
    let mut operation = Box::pin(maestro_agent::run_agent_loop_continue(
        context.clone(),
        &config,
        sink(),
        options,
    ));
    assert!(operation.as_mut().now_or_never().is_none());
    context.messages = Arc::new(std::sync::RwLock::new(vec![user("replacement slot")]));
    assert_eq!(history.read().unwrap().len(), if started { 2 } else { 1 });
    let final_message = assistant("published later");
    stream.end(Some(Arc::clone(&final_message)));
    let returned = operation.await.unwrap();
    let AgentMessage::Assistant(message) = &returned[0] else {
        panic!("assistant result");
    };
    assert!(Arc::ptr_eq(message, &final_message));
    let history = history.read().unwrap();
    assert_eq!(history.len(), 2);
    let AgentMessage::Assistant(stored) = &history[1] else {
        panic!("final history assistant");
    };
    assert!(Arc::ptr_eq(stored, &final_message));
    assert_eq!(context.messages.read().unwrap().len(), 1);
}
/// Gate one chosen event and verify no later phase passes its awaited barrier.
async fn event_barrier_case(target: &'static str) {
    use futures_util::FutureExt;
    let (release, released) = tokio::sync::oneshot::channel();
    let released = Arc::new(Mutex::new(Some(released)));
    let seen = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&seen);
    let emit = Arc::new(move |event| {
        let name = kind(&event);
        observed.lock().unwrap().push(name);
        let gate = if name == target {
            released.lock().unwrap().take()
        } else {
            None
        };
        if let Some(gate) = gate {
            Box::pin(async {
                gate.await.unwrap();
                Ok(())
            }) as maestro_models::BoxFuture<_>
        } else {
            ready(Ok(()))
        }
    });
    let context = with_execute(Arc::new(|_, _, _, update| {
        update.unwrap()(result());
        ready(Ok(result()))
    }));
    let config = config();
    let mut options = responses(vec![tool_call()]);
    if target == "message_update" {
        options = update_stream_options();
    }
    let mut operation = Box::pin(maestro_agent::run_agent_loop_continue(
        context, &config, emit, options,
    ));
    assert!(operation.as_mut().now_or_never().is_none());
    assert_eq!(seen.lock().unwrap().last(), Some(&target));
    release.send(()).unwrap();
    assert!(operation.await.is_ok());
}
/// Supply one started model update followed by a terminating tool call.
fn update_stream_options() -> maestro_agent::AgentLoopOptions {
    maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(|_, _, _, _| {
            let stream = maestro_models::AssistantMessageEventStream::new();
            let partial = assistant("partial");
            stream.push(maestro_models::AssistantMessageEvent::Start {
                partial: Arc::clone(&partial),
            });
            stream.push(maestro_models::AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "x".into(),
                partial,
            });
            stream.end(Some(tool_call()));
            ready(Ok(stream))
        })),
    }
}

/// Construct ignored and admitted updates with a shared live partial.
fn snapshot_stream_options(
    live: maestro_models::SharedAssistantMessage,
) -> maestro_agent::AgentLoopOptions {
    use maestro_models::AssistantMessageEvent;
    maestro_agent::AgentLoopOptions {
        signal: None,
        stream_fn: Some(Arc::new(move |_, _, _, _| {
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.push(AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "ignored".into(),
                partial: Arc::clone(&live),
            });
            stream.push(AssistantMessageEvent::Start {
                partial: Arc::clone(&live),
            });
            stream.push(AssistantMessageEvent::TextDelta {
                content_index: 0,
                delta: "part".into(),
                partial: Arc::clone(&live),
            });
            stream.end(Some(assistant("final")));
            Box::pin(async { Ok(stream) })
        })),
    }
}

/// Invocation cancellation shares its signal through transform, model and tool.
async fn shared_signal_case() {
    let invocation = maestro_models::Cancellation::new();
    let configured = maestro_models::Cancellation::new();
    let mut config = config();
    config.options.common.signal = Some(configured.clone());
    config.transform_context = Some(Arc::new(|history, signal| {
        signal.unwrap().abort();
        ready(Ok(history))
    }));
    let context = with_execute(Arc::new(|_, _, signal, _| {
        assert!(signal.unwrap().is_aborted());
        ready(Ok(result()))
    }));
    let options = maestro_agent::AgentLoopOptions {
        signal: Some(invocation.clone()),
        stream_fn: Some(Arc::new(|_, _, options, open| {
            assert!(open.common.signal.unwrap().is_aborted());
            assert!(options.common.signal.unwrap().is_aborted());
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(tool_call()));
            ready(Ok(stream))
        })),
    };
    assert!(
        maestro_agent::run_agent_loop_continue(context, &config, sink(), options)
            .await
            .is_ok()
    );
    assert!(invocation.is_aborted());
    assert!(!configured.is_aborted());
}
/// Observe no events while retaining awaited success.
fn sink() -> maestro_agent::AgentEventSink {
    Arc::new(|_| Box::pin(async { Ok(()) }))
}
