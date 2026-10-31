//! Configuration capture and request binding through the public owner.
#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
use crate::support;
use maestro_agent::{Agent, AgentOptions};
use maestro_models::ThinkingBudgets;
use std::sync::{Arc, Mutex, RwLock};

/// Retained budgets are read after key resolution on every turn.
#[test]
fn captured_budget_edits_reach_each_request() {
    support::run(async {
        support::binding_corpus::retained().await;
        let mut config = support::config();
        config.options.thinking_budgets = Some(ThinkingBudgets {
            high: Some(17.0),
            ..Default::default()
        });
        let seen = Arc::new(Mutex::new(None));
        let recorded = seen.clone();
        let options = maestro_agent::AgentLoopOptions {
            signal: None,
            stream_fn: Some(Arc::new(move |_, _, options, _| {
                *recorded.lock().unwrap() = options.thinking_budgets;
                let stream = maestro_models::AssistantMessageEventStream::new();
                stream.end(Some(support::assistant("done")));
                support::ready(Ok(stream))
            })),
        };
        maestro_agent::run_agent_loop(
            vec![],
            support::context(),
            &config,
            Arc::new(|_| support::ready(Ok(()))),
            options,
        )
        .await
        .unwrap();
        assert_eq!(seen.lock().unwrap().as_ref().unwrap().high, Some(17.0));
    });
}

#[test]
fn construction_defers_callbacks_and_installs_defaults() {
    let tasks = Arc::new(support::owned::Tasks::default());
    let calls = Arc::new(Mutex::new(Vec::new()));
    let convert_calls = calls.clone();
    let transform_calls = calls.clone();
    let key_calls = calls.clone();
    let agent: Agent = Agent::new(AgentOptions {
        convert_to_llm: Some(Arc::new(move |history| {
            convert_calls.lock().unwrap().push("convert");
            assert_eq!(history.read().unwrap().len(), 2);
            let maestro_agent::AgentMessage::User(last) = &history.read().unwrap()[1] else {
                panic!("expected transformed user");
            };
            assert_eq!(
                last.read().unwrap().content,
                maestro_models::UserContent::Text("transformed".into())
            );
            support::ready(Ok(vec![maestro_models::Message::User(
                maestro_models::UserMessage {
                    content: maestro_models::UserContent::Text("converted".into()),
                    timestamp: 3.0,
                },
            )]))
        })),
        transform_context: Some(Arc::new(move |history, _| {
            transform_calls.lock().unwrap().push("transform");
            history.write().unwrap().push(support::user("transformed"));
            support::ready(Ok(history))
        })),
        get_api_key: Some(Arc::new(move |_| {
            key_calls.lock().unwrap().push("key");
            support::ready(Ok(Some("constructor-key".into())))
        })),
        spawn: Some(tasks.spawner()),
        stream_fn: Some(Arc::new(constructor_dispatch)),
        session_id: Some(String::new()),
        transport: Some(maestro_models::Transport::Sse),
        max_retry_delay_ms: Some(-0.5),
        tool_execution: Some(maestro_agent::ToolExecutionMode::Sequential),
        ..constructor_hooks(&calls)
    });
    assert!(calls.lock().unwrap().is_empty());
    assert_eq!(agent.session_id.read().unwrap().as_deref(), Some(""));
    let defaults: Agent = Agent::default();
    support::binding_corpus::compare(
        "inert_construction",
        None,
        serde_json::json!({"calls":*calls.lock().unwrap(),"transport":*defaults.transport.read().unwrap(),"execution":if *defaults.tool_execution.read().unwrap()==maestro_agent::ToolExecutionMode::Parallel {"parallel"}else{"sequential"},"sessionAbsent":defaults.session_id.read().unwrap().is_none(),"budgetsAbsent":defaults.thinking_budgets.read().unwrap().is_none(),"retryAbsent":defaults.max_retry_delay_ms.read().unwrap().is_none(),"streaming":agent.state().read().unwrap().is_streaming()}),
    );
    support::run(drive_constructor(agent, tasks, calls));
}

/// Configure payload, response and tool callbacks before constructing their owner.
fn constructor_hooks(calls: &Arc<Mutex<Vec<&'static str>>>) -> AgentOptions {
    let payload_calls = calls.clone();
    let response_calls = calls.clone();
    let before_calls = calls.clone();
    let after_calls = calls.clone();
    AgentOptions {
        on_payload: Some(Arc::new(move |mut payload, _| {
            payload_calls.lock().unwrap().push("payload");
            payload.payload["constructor"] = serde_json::json!(true);
            support::ready(Ok(payload))
        })),
        on_response: Some(Arc::new(move |response, _| {
            assert_eq!(response.status.to_bits(), 201.0_f64.to_bits());
            response_calls.lock().unwrap().push("response");
            support::ready(Ok(()))
        })),
        before_tool_call: Some(Arc::new(move |context, _| {
            context.args["constructor"] = serde_json::json!("before");
            before_calls.lock().unwrap().push("before");
            support::ready(Ok(None))
        })),
        after_tool_call: Some(Arc::new(move |context, _| {
            assert_eq!(context.result.details, Some(serde_json::json!("executed")));
            after_calls.lock().unwrap().push("after");
            support::ready(Ok(Some(maestro_agent::AfterToolCallResult {
                details: Some(serde_json::json!("after")),
                terminate: Some(true),
                ..Default::default()
            })))
        })),
        ..Default::default()
    }
}

/// Observe constructor-bound conversion and forwarded provider callbacks at dispatch.
fn constructor_dispatch(
    model: maestro_models::Model,
    context: maestro_models::Context,
    options: maestro_models::SimpleStreamOptions,
    _: maestro_models::ProviderStreamOptions,
) -> maestro_models::BoxFuture<
    Result<maestro_models::AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>,
> {
    Box::pin(async move {
        assert_eq!(context.messages.len(), 1);
        let maestro_models::Message::User(user) = &context.messages[0] else {
            panic!("expected converted user");
        };
        assert_eq!(
            user.content,
            maestro_models::UserContent::Text("converted".into())
        );
        assert_eq!(options.common.api_key.as_deref(), Some("constructor-key"));
        let model = Arc::new(model);
        let payload = options.common.on_payload.unwrap()(
            maestro_models::PayloadHookInput {
                payload: serde_json::json!({"original":true}),
                signal: None,
            },
            model.clone(),
        )
        .await?;
        assert_eq!(
            payload.payload,
            serde_json::json!({"original":true,"constructor":true})
        );
        options.common.on_response.unwrap()(
            maestro_models::ProviderResponse {
                status: 201.0,
                headers: std::collections::BTreeMap::default(),
            },
            model,
        )
        .await?;
        let stream = maestro_models::AssistantMessageEventStream::new();
        stream.end(Some(support::tool_call()));
        Ok(stream)
    })
}

/// Join the same constructor-configured run through tool execution and final reduction.
async fn drive_constructor(
    agent: Agent,
    tasks: Arc<support::owned::Tasks>,
    calls: Arc<Mutex<Vec<&'static str>>>,
) {
    let executed = calls.clone();
    let tools = support::with_execute(Arc::new(move |_, args, _, _| {
        assert_eq!(args, serde_json::json!({"constructor":"before"}));
        executed.lock().unwrap().push("execute");
        support::ready(Ok(maestro_agent::AgentToolResult {
            details: Some(serde_json::json!("executed")),
            terminate: None,
            content: vec![],
        }))
    }))
    .tools
    .unwrap();
    agent
        .state()
        .write()
        .unwrap()
        .set_tools(tools.read().unwrap().clone());
    let agent = Arc::new(agent);
    let observe = agent.prompt(support::owned::input("original"));
    tasks.take().await;
    observe.await.unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        [
            "transform",
            "convert",
            "key",
            "payload",
            "response",
            "before",
            "execute",
            "after"
        ]
    );
    let messages = agent
        .state()
        .read()
        .unwrap()
        .messages()
        .read()
        .unwrap()
        .clone();
    let maestro_agent::AgentMessage::ToolResult(result) = messages.last().unwrap() else {
        panic!("expected final tool result");
    };
    assert_eq!(
        result.read().unwrap().details,
        Some(serde_json::json!("after"))
    );
}

#[test]
fn admitted_runs_keep_callbacks_and_scalar_slots() {
    support::run(async {
        let (agent, tasks) = support::owned::agent();
        let seen = Arc::new(Mutex::new(Vec::new()));
        *agent.session_id.write().unwrap() = Some("old".into());
        let recorded = seen.clone();
        *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, _, options, open| {
            recorded.lock().unwrap().push((
                options.common.session_id,
                open.extra["toolExecution"].clone(),
            ));
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(support::assistant("done")));
            support::ready(Ok(stream))
        }));
        let observe = agent.prompt(support::owned::input("start"));
        *agent.session_id.write().unwrap() = Some("next".into());
        *agent.tool_execution.write().unwrap() = maestro_agent::ToolExecutionMode::Sequential;
        tasks.take().await;
        observe.await.unwrap();
        assert_eq!(
            *seen.lock().unwrap(),
            vec![(Some("old".into()), serde_json::json!("parallel"))]
        );
        let observe = agent.prompt(support::owned::input("next"));
        tasks.take().await;
        observe.await.unwrap();
        assert_eq!(
            &seen.lock().unwrap()[1..],
            &[(Some("next".into()), serde_json::json!("sequential"))]
        );
        support::binding_corpus::captured().await;
    });
}

/// Full recorded configuration input and outcome projection.
#[test]
fn request_options_preserve_all_declared_values() {
    support::run(support::continuation_corpus::replay("R3"));
}

/// Full recorded configuration input and outcome projection.
#[test]
fn credentials_refresh_after_transform_and_conversion() {
    support::run(async {
        support::continuation_corpus::replay("R4").await;
        shared_transform_signal().await;
    });
}

/// Full recorded configuration input and outcome projection.
#[test]
fn configuration_failures_follow_their_owning_catch_paths() {
    support::run(async {
        support::continuation_corpus::replay("R6").await;
        let (agent, tasks) = support::owned::agent();
        *agent.transform_context.write().unwrap() = Some(Arc::new(|_, _| {
            support::ready(Err(support::owned::failure("transform\nfailed")))
        }));
        agent.subscribe(Arc::new(|event, _| {
            let result = if matches!(
                &*event.read().unwrap(),
                maestro_agent::AgentEvent::AgentEnd { .. }
            ) {
                Err(support::owned::failure("end\nfailed"))
            } else {
                Ok(())
            };
            support::ready(result)
        }));
        let observe = agent.prompt(support::owned::input("failure"));
        tasks.take().await;
        assert_eq!(observe.await.unwrap_err().message, "end\nfailed");
        assert_eq!(
            agent.state().read().unwrap().error_message(),
            Some("transform\nfailed")
        );
        agent.wait_for_idle().await;
        assert!(agent.signal().is_none());
    });
}

/// Captured model edits stay live across credential waiting and provider refresh.
#[test]
fn key_barrier_preserves_selected_model_identity() {
    support::run(async {
        let (agent, tasks) = support::owned::agent();
        let model = Arc::new(RwLock::new(support::model()));
        model.write().unwrap().provider = "selected-before".into();
        agent.state().write().unwrap().model = model.clone();
        let providers = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (entered, entry) = tokio::sync::oneshot::channel();
        let (release, released) = tokio::sync::oneshot::channel();
        let gates = Mutex::new(Some((entered, released)));
        let recorded = providers.clone();
        *agent.get_api_key.write().unwrap() = Some(Arc::new(move |provider| {
            recorded.lock().unwrap().push(provider);
            let gate = gates.lock().unwrap().take();
            Box::pin(async move {
                support::wait_gate(gate).await;
                Ok(None)
            })
        }));
        let recorded = seen.clone();
        *agent.stream_fn.write().unwrap() = Some(Arc::new(move |model, _, _, _| {
            recorded.lock().unwrap().push((model.provider, model.id));
            let stream = maestro_models::AssistantMessageEventStream::new();
            stream.end(Some(support::assistant("done")));
            support::ready(Ok(stream))
        }));
        agent.follow_up(support::user("second"));
        let observe = agent.prompt(support::owned::input("first"));
        let joined = tokio::spawn(tasks.take());
        entry.await.unwrap();
        agent.state().write().unwrap().model = Arc::new(RwLock::new(support::model()));
        model.write().unwrap().provider = "selected-after".into();
        model.write().unwrap().id = "edited".into();
        release.send(()).unwrap();
        joined.await.unwrap();
        observe.await.unwrap();
        assert_eq!(
            *providers.lock().unwrap(),
            ["selected-before", "selected-after"]
        );
        assert_eq!(
            *seen.lock().unwrap(),
            [
                ("selected-after".into(), "edited".into()),
                ("selected-after".into(), "edited".into())
            ]
        );
    });
}

/// A callback payload whose final destructor reenters Agent observations.
struct Retired {
    /// Owner to observe without retaining it.
    owner: std::sync::Weak<Agent>,
    /// Completed destructor witness.
    dropped: Arc<std::sync::atomic::AtomicUsize>,
}
impl Drop for Retired {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.upgrade() {
            assert!(owner.signal().is_none());
            assert!(owner.state().try_write().is_ok());
            assert!(owner.convert_to_llm.try_write().is_ok());
        }
        self.dropped
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
}
/// Task settlement releases captured callbacks outside internal guards.
#[test]
fn captured_callbacks_release_after_settlement_and_slot_replacement() {
    support::run(async {
        let (agent, tasks) = support::owned::agent();
        *agent.stream_fn.write().unwrap() = support::responses(vec![
            support::assistant("first"),
            support::assistant("cycle"),
        ])
        .stream_fn;
        let dropped = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let payload = Arc::new(Retired {
            owner: Arc::downgrade(&agent),
            dropped: dropped.clone(),
        });
        let weak = Arc::downgrade(&payload);
        retaining_callbacks(&agent, &payload);
        let observe = agent.prompt(support::owned::input("start"));
        let defaults: Agent = Agent::default();
        *agent.convert_to_llm.write().unwrap() = defaults.convert_to_llm.read().unwrap().clone();
        *agent.transform_context.write().unwrap() = None;
        *agent.get_api_key.write().unwrap() = None;
        *agent.on_payload.write().unwrap() = None;
        *agent.on_response.write().unwrap() = None;
        *agent.before_tool_call.write().unwrap() = None;
        *agent.after_tool_call.write().unwrap() = None;
        drop(payload);
        assert!(weak.upgrade().is_some());
        tasks.take().await;
        observe.await.unwrap();
        assert!(weak.upgrade().is_none());
        assert_eq!(dropped.load(std::sync::atomic::Ordering::Relaxed), 1);
        let owner = agent.clone();
        let converter = agent.convert_to_llm.read().unwrap().clone();
        *agent.convert_to_llm.write().unwrap() = Arc::new(move |history| {
            assert!(owner.state().try_read().is_ok());
            converter(history)
        });
        let observe = agent.prompt(support::owned::input("cycle"));
        *agent.convert_to_llm.write().unwrap() = defaults.convert_to_llm.read().unwrap().clone();
        let weak_owner = Arc::downgrade(&agent);
        drop(agent);
        assert!(weak_owner.upgrade().is_some());
        tasks.take().await;
        observe.await.unwrap();
        assert!(weak_owner.upgrade().is_none());
    });
}

/// Retain a single observed payload through each changed callback category.
fn retaining_callbacks(agent: &Agent, payload: &Arc<Retired>) {
    let keep = payload.clone();
    let converter = agent.convert_to_llm.read().unwrap().clone();
    *agent.convert_to_llm.write().unwrap() = Arc::new(move |history| {
        let _keep = &keep;
        converter(history)
    });
    let keep = payload.clone();
    *agent.transform_context.write().unwrap() = Some(Arc::new(move |history, _| {
        let _keep = &keep;
        support::ready(Ok(history))
    }));
    let keep = payload.clone();
    *agent.get_api_key.write().unwrap() = Some(Arc::new(move |_| {
        let _keep = &keep;
        support::ready(Ok(None))
    }));
    let keep = payload.clone();
    *agent.on_payload.write().unwrap() = Some(Arc::new(move |p, _| {
        let _keep = &keep;
        support::ready(Ok(p))
    }));
    let keep = payload.clone();
    *agent.on_response.write().unwrap() = Some(Arc::new(move |_, _| {
        let _keep = &keep;
        support::ready(Ok(()))
    }));
    let keep = payload.clone();
    *agent.before_tool_call.write().unwrap() = Some(Arc::new(move |_, _| {
        let _keep = &keep;
        support::ready(Ok(None))
    }));
    let keep = payload.clone();
    *agent.after_tool_call.write().unwrap() = Some(Arc::new(move |_, _| {
        let _keep = &keep;
        support::ready(Ok(None))
    }));
}

/// Cancellation through the owner reaches both transformation and dispatch handles.
async fn shared_transform_signal() {
    let (agent, tasks) = support::owned::agent();
    let seen = Arc::new(Mutex::new(None));
    let captured = seen.clone();
    *agent.transform_context.write().unwrap() = Some(Arc::new(move |history, signal| {
        assert!(!signal.as_ref().unwrap().is_aborted());
        *captured.lock().unwrap() = signal;
        support::ready(Ok(history))
    }));
    let owner = Arc::downgrade(&agent);
    let captured = seen.clone();
    *agent.stream_fn.write().unwrap() = Some(Arc::new(move |_, _, options, _| {
        assert!(!options.common.signal.as_ref().unwrap().is_aborted());
        owner.upgrade().unwrap().abort();
        assert!(captured.lock().unwrap().as_ref().unwrap().is_aborted());
        assert!(options.common.signal.unwrap().is_aborted());
        let stream = maestro_models::AssistantMessageEventStream::new();
        let response = support::assistant("aborted");
        response.write().unwrap().stop_reason = maestro_models::StopReason::Aborted;
        stream.end(Some(response));
        support::ready(Ok(stream))
    }));
    let observe = agent.prompt(support::owned::input("signal"));
    tasks.take().await;
    observe.await.unwrap();
}
