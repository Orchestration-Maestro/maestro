use maestro_models::*;
use std::sync::{Arc, Mutex};

static REGISTRY: Mutex<()> = Mutex::new(());

fn model(api: &str) -> Model {
    Model {
        id: "unregistered".into(),
        name: String::new(),
        api: api.into(),
        provider: "unregistered".into(),
        base_url: String::new(),
        reasoning: false,
        thinking_level_map: None,
        input: vec![],
        cost: TokenRates {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: -1.5,
        max_tokens: 0.0,
        headers: None,
        compat: None,
    }
}

#[test]
fn descriptor_invocation_uses_api_and_caller_fields() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    let supplied = model("custom");
    let context = Context {
        system_prompt: Some(String::new()),
        messages: vec![Message::ToolResult(ToolResultMessage {
            tool_call_id: "call".into(),
            tool_name: "tool".into(),
            content: vec![],
            details: Some(serde_json::json!({"private": true})),
            is_error: false,
            timestamp: -2.5,
        })],
        tools: Some(vec![]),
    };
    let seen = Arc::new(Mutex::new(None));
    let output = seen.clone();
    register_api_provider(
        ApiProvider {
            api: "custom".into(),
            stream: Arc::new(move |model, context, options| {
                *output.lock().unwrap() = Some((model, context, options));
                Ok(create_assistant_message_event_stream())
            }),
            stream_simple: Arc::new(|_, _, _| panic!("wrong callback")),
        },
        None,
    );
    let _stream = stream(supplied.clone(), context.clone(), None).unwrap();
    let (actual, actual_context, options) = seen.lock().unwrap().take().unwrap();
    assert_eq!(actual, supplied);
    assert_eq!(actual_context, context);
    assert!(options.is_none());
    clear_api_providers();
}

fn context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![],
        tools: None,
    }
}
fn provider(api: &str) -> ApiProvider {
    ApiProvider {
        api: api.into(),
        stream: Arc::new(|_, _, _| Ok(AssistantMessageEventStream::new())),
        stream_simple: Arc::new(|_, _, _| Ok(AssistantMessageEventStream::new())),
    }
}
#[test]
fn api_registry_replacement_keeps_position_and_source() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    register_api_provider(provider("a"), Some("old".into()));
    register_api_provider(provider("b"), None);
    let retained = get_api_provider("a").unwrap();
    let replacement_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let raw_calls = replacement_calls.clone();
    let simple_calls = replacement_calls.clone();
    let replacement = ApiProvider {
        api: "a".into(),
        stream: Arc::new(move |_, _, _| {
            raw_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(AssistantMessageEventStream::new())
        }),
        stream_simple: Arc::new(move |_, _, _| {
            simple_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(AssistantMessageEventStream::new())
        }),
    };
    register_api_provider(replacement, Some("new".into()));
    stream(model("a"), context(), None).unwrap();
    stream_simple(model("a"), context(), None).unwrap();
    assert_eq!(
        replacement_calls.load(std::sync::atomic::Ordering::SeqCst),
        2
    );
    assert_eq!(
        get_api_providers()
            .iter()
            .map(|p| p.api.as_str())
            .collect::<Vec<_>>(),
        ["a", "b"]
    );
    unregister_api_providers("old");
    assert!(get_api_provider("a").is_some());
    unregister_api_providers("new");
    assert!(get_api_provider("a").is_none());
    (retained.stream)(model("a"), context(), None).unwrap();
    register_api_provider(provider("a"), Some(String::new()));
    unregister_api_providers("missing");
    unregister_api_providers("");
    assert_eq!(get_api_providers().len(), 1);
    register_api_provider(provider("a"), None);
    assert_eq!(
        get_api_providers()
            .iter()
            .map(|p| p.api.as_str())
            .collect::<Vec<_>>(),
        ["b", "a"]
    );
    clear_api_providers();
    clear_api_providers();
    assert!(get_api_providers().is_empty());
}

fn text_error(value: ThrownValue) -> String {
    match value {
        ThrownValue::Error(e) => e.message,
        _ => panic!("expected error instance"),
    }
}
#[test]
fn wrappers_reject_other_api_before_adapter() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let raw = calls.clone();
    let simple = calls.clone();
    register_api_provider(
        ApiProvider {
            api: "right".into(),
            stream: Arc::new(move |_, _, _| {
                raw.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(AssistantMessageEventStream::new())
            }),
            stream_simple: Arc::new(move |_, _, _| {
                simple.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(AssistantMessageEventStream::new())
            }),
        },
        None,
    );
    let callbacks = get_api_provider("right").unwrap();
    assert_eq!(
        text_error(
            (callbacks.stream)(model("wrong"), context(), None)
                .err()
                .unwrap()
        ),
        "Mismatched api: wrong expected right"
    );
    assert_eq!(
        text_error(
            (callbacks.stream_simple)(model("wrong"), context(), None)
                .err()
                .unwrap()
        ),
        "Mismatched api: wrong expected right"
    );
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    for api in ["", "arbitrary/协议"] {
        register_api_provider(provider(api), None);
        stream(model(api), context(), None).unwrap();
    }
    clear_api_providers();
}

#[test]
fn raw_simple_calls_forward_options_unchanged() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    let seen = Arc::new(Mutex::new(vec![]));
    let raw = seen.clone();
    let simple = seen.clone();
    register_api_provider(
        ApiProvider {
            api: "options".into(),
            stream: Arc::new(move |_, _, o| {
                raw.lock()
                    .unwrap()
                    .push(o.map(|o| serde_json::to_value(o).unwrap()));
                Ok(AssistantMessageEventStream::new())
            }),
            stream_simple: Arc::new(move |_, _, o| {
                simple
                    .lock()
                    .unwrap()
                    .push(o.map(|o| serde_json::to_value(o).unwrap()));
                Ok(AssistantMessageEventStream::new())
            }),
        },
        None,
    );
    stream(model("options"), context(), None).unwrap();
    stream_simple(model("options"), context(), None).unwrap();
    let raw = ProviderStreamOptions {
        base: StreamOptions {
            temperature: Some(0.0),
            max_tokens: Some(-0.5),
            api_key: Some(String::new()),
            headers: Some(Default::default()),
            max_retries: Some(0.0),
            timeout_ms: Some(123.0),
            max_retry_delay_ms: Some(456.0),
            ..Default::default()
        },
        extra: serde_json::json!({"custom":false,"null":null})
            .as_object()
            .unwrap()
            .clone(),
    };
    let simple = SimpleStreamOptions {
        base: StreamOptions {
            session_id: Some(String::new()),
            ..Default::default()
        },
        reasoning: Some(ThinkingLevel::Minimal),
        thinking_budgets: Some(ThinkingBudgets {
            minimal: Some(0.0),
            low: Some(1.0),
            medium: Some(2.0),
            high: Some(3.0),
        }),
    };
    let expected_raw = serde_json::to_value(&raw).unwrap();
    let expected_simple = serde_json::to_value(&simple).unwrap();
    assert_eq!(expected_raw["timeoutMs"], 123.0);
    assert_eq!(expected_raw["maxRetryDelayMs"], 456.0);
    assert_eq!(
        expected_simple["thinkingBudgets"],
        serde_json::json!({"minimal":0.0,"low":1.0,"medium":2.0,"high":3.0})
    );
    stream(model("options"), context(), Some(raw.clone())).unwrap();
    stream_simple(model("options"), context(), Some(simple.clone())).unwrap();
    drop(complete(model("options"), context(), Some(raw)));
    drop(complete_simple(model("options"), context(), Some(simple)));
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            None,
            None,
            Some(expected_raw.clone()),
            Some(expected_simple.clone()),
            Some(expected_raw),
            Some(expected_simple)
        ]
    );
    clear_api_providers();
}

fn assistant() -> Arc<std::sync::RwLock<AssistantMessage>> {
    Arc::new(std::sync::RwLock::new(AssistantMessage {
        content: vec![],
        api: "custom".into(),
        provider: "caller".into(),
        model: "id".into(),
        response_model: None,
        response_id: None,
        diagnostics: None,
        usage: Usage {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total_tokens: 0.0,
            cost: UsageCost {
                input: 0.0,
                output: 0.0,
                cache_read: 0.0,
                cache_write: 0.0,
                total: 0.0,
            },
        },
        stop_reason: StopReason::Stop,
        error_message: None,
        timestamp: 0.0,
    }))
}
struct Counter(std::sync::atomic::AtomicUsize);
impl std::task::Wake for Counter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}
fn poll<T>(
    f: &mut std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + '_>>,
) -> std::task::Poll<T> {
    let waker = std::task::Waker::from(Arc::new(Counter(std::sync::atomic::AtomicUsize::new(0))));
    f.as_mut().poll(&mut std::task::Context::from_waker(&waker))
}
fn ready<T>(f: &mut std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + '_>>) -> T {
    match poll(f) {
        std::task::Poll::Ready(v) => v,
        _ => panic!("expected ready"),
    }
}
#[test]
fn complete_starts_once_without_consuming_events() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    let streams = Arc::new(Mutex::new(vec![]));
    let raw = streams.clone();
    let simple = streams.clone();
    register_api_provider(
        ApiProvider {
            api: "eager".into(),
            stream: Arc::new(move |_, _, _| {
                let s = AssistantMessageEventStream::new();
                raw.lock().unwrap().push(s.clone());
                Ok(s)
            }),
            stream_simple: Arc::new(move |_, _, _| {
                let s = AssistantMessageEventStream::new();
                simple.lock().unwrap().push(s.clone());
                Ok(s)
            }),
        },
        None,
    );
    let futures = [
        complete(model("eager"), context(), None),
        complete_simple(model("eager"), context(), None),
    ];
    assert_eq!(streams.lock().unwrap().len(), 2);
    for (mut future, stream) in futures.into_iter().zip(streams.lock().unwrap().iter()) {
        let message = assistant();
        stream
            .push(AssistantMessageEvent::Start {
                partial: message.clone(),
            })
            .unwrap();
        stream
            .push(AssistantMessageEvent::Done {
                reason: StopReason::Stop,
                message: message.clone(),
            })
            .unwrap();
        assert!(Arc::ptr_eq(&ready(&mut future).unwrap(), &message));
        assert!(Arc::ptr_eq(&ready(&mut stream.result()), &message));
        let mut iter = stream.iter();
        assert!(matches!(
            ready(&mut iter.next()),
            Some(AssistantMessageEvent::Start { .. })
        ));
        assert!(matches!(
            ready(&mut iter.next()),
            Some(AssistantMessageEvent::Done { .. })
        ));
        assert!(ready(&mut iter.next()).is_none());
    }
    assert_eq!(streams.lock().unwrap().len(), 2);
    clear_api_providers();
}

#[test]
fn lookup_setup_and_stream_errors_keep_their_boundaries() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    assert_eq!(
        text_error(stream(model("missing"), context(), None).err().unwrap()),
        "No API provider registered for api: missing"
    );
    assert_eq!(
        text_error(
            ready(&mut complete(model("missing"), context(), None))
                .err()
                .unwrap()
        ),
        "No API provider registered for api: missing"
    );
    let signal = Cancellation::new();
    signal.cancel();
    register_api_provider(
        ApiProvider {
            api: "errors".into(),
            stream: Arc::new(|_, _, o| {
                assert!(o.unwrap().base.signal.unwrap().is_cancelled());
                Err(ThrownValue::Json(serde_json::json!("exact setup")))
            }),
            stream_simple: Arc::new(|_, _, _| Err(ThrownValue::Undefined)),
        },
        None,
    );
    let o = ProviderStreamOptions {
        base: StreamOptions {
            signal: Some(signal),
            ..Default::default()
        },
        ..Default::default()
    };
    assert!(
        matches!(stream(model("errors"), context(), Some(o.clone())), Err(ThrownValue::Json(v)) if v == "exact setup")
    );
    assert!(
        matches!(ready(&mut complete(model("errors"), context(), Some(o))), Err(ThrownValue::Json(v)) if v == "exact setup")
    );
    let s = AssistantMessageEventStream::new();
    let message = assistant();
    s.push(AssistantMessageEvent::Error {
        reason: StopReason::Error,
        error: message.clone(),
    })
    .unwrap();
    assert!(Arc::ptr_eq(&ready(&mut s.result()), &message));
    clear_api_providers();
}

fn queue() -> EventStream<i32> {
    EventStream::new(Arc::new(|v| Ok(*v < 0)), Arc::new(|v| Ok(*v)))
}
#[test]
fn queue_delivery_is_fifo_one_waiter_at_a_time() {
    let s = queue();
    let mut a = s.iter();
    let mut b = s.iter();
    let mut first = a.next();
    let mut second = b.next();
    assert!(poll(&mut first).is_pending());
    assert!(poll(&mut second).is_pending());
    s.push(1).unwrap();
    assert_eq!(ready(&mut first), Some(1));
    assert!(poll(&mut second).is_pending());
    s.push(2).unwrap();
    assert_eq!(ready(&mut second), Some(2));
    drop(first);
    drop(second);
    s.push(3).unwrap();
    s.push(4).unwrap();
    assert_eq!(ready(&mut a.next()), Some(3));
    assert_eq!(ready(&mut a.next()), Some(4));
    assert!(poll(&mut s.result()).is_pending());
    let s = queue();
    let mut cursor = s.iter();
    drop(cursor.next());
    drop(cursor.next());
    let mut third = cursor.next();
    s.push(1).unwrap();
    s.push(2).unwrap();
    s.push(3).unwrap();
    assert_eq!(ready(&mut third), Some(3));
}

#[test]
fn terminal_push_settles_first_result_and_ignores_later_push() {
    for value in [
        serde_json::json!(false),
        serde_json::json!(0),
        serde_json::json!(""),
        serde_json::Value::Null,
    ] {
        let s = EventStream::new(
            Arc::new(|_: &serde_json::Value| Ok(true)),
            Arc::new(|v| Ok(v.clone())),
        );
        let mut before = s.result();
        assert!(poll(&mut before).is_pending());
        s.push(value.clone()).unwrap();
        s.push(serde_json::json!("later")).unwrap();
        s.end(Some(serde_json::json!("new")));
        s.end(None);
        assert_eq!(ready(&mut before), value);
        assert_eq!(ready(&mut s.result()), value);
        let mut iter = s.iter();
        assert_eq!(ready(&mut iter.next()), Some(value));
        assert_eq!(ready(&mut iter.next()), None);
    }
    for reason in [StopReason::Stop, StopReason::Error] {
        let s = AssistantMessageEventStream::new();
        let a = assistant();
        let mut iter = s.iter();
        let mut waiter = iter.next();
        let e = if reason == StopReason::Stop {
            AssistantMessageEvent::Done {
                reason,
                message: a.clone(),
            }
        } else {
            AssistantMessageEvent::Error {
                reason,
                error: a.clone(),
            }
        };
        s.push(e).unwrap();
        assert!(ready(&mut waiter).is_some());
        assert!(Arc::ptr_eq(&ready(&mut s.result()), &a));
    }
}

#[test]
fn end_wakes_iterators_but_can_leave_result_pending() {
    let s = queue();
    let mut a = s.iter();
    let mut b = s.iter();
    let mut first = a.next();
    let mut second = b.next();
    let wakes = Arc::new(Counter(std::sync::atomic::AtomicUsize::new(0)));
    let waker = std::task::Waker::from(wakes.clone());
    let mut cx = std::task::Context::from_waker(&waker);
    assert!(first.as_mut().poll(&mut cx).is_pending());
    assert!(second.as_mut().poll(&mut cx).is_pending());
    s.end(None);
    assert_eq!(wakes.0.load(std::sync::atomic::Ordering::SeqCst), 2);
    assert_eq!(ready(&mut first), None);
    assert_eq!(ready(&mut second), None);
    assert!(poll(&mut s.result()).is_pending());
    s.push(-1).unwrap();
    assert!(poll(&mut s.result()).is_pending());
    s.end(Some(0));
    assert_eq!(ready(&mut s.result()), 0);
    let s = queue();
    s.push(1).unwrap();
    s.end(None);
    let mut iter = s.iter();
    assert_eq!(ready(&mut iter.next()), Some(1));
    assert_eq!(ready(&mut iter.next()), None);
}

#[test]
fn shared_partials_keep_late_signatures_and_replacements() {
    let s = AssistantMessageEventStream::new();
    let a = assistant();
    let call = Arc::new(std::sync::RwLock::new(ToolCall {
        id: "id".into(),
        name: "tool".into(),
        arguments: Default::default(),
        thought_signature: None,
    }));
    a.write().unwrap().content = vec![
        AssistantContent::Text(TextContent {
            text: "old".into(),
            text_signature: None,
        }),
        AssistantContent::Thinking(ThinkingContent {
            thinking: "reason".into(),
            thinking_signature: None,
            redacted: None,
        }),
        AssistantContent::ToolCall(call.clone()),
    ];
    s.push(AssistantMessageEvent::Start { partial: a.clone() })
        .unwrap();
    s.push(AssistantMessageEvent::TextDelta {
        content_index: -99.5,
        delta: "unrelated".into(),
        partial: a.clone(),
    })
    .unwrap();
    s.push(AssistantMessageEvent::ToolcallEnd {
        content_index: 40.5,
        tool_call: call.clone(),
        partial: a.clone(),
    })
    .unwrap();
    {
        let mut a = a.write().unwrap();
        a.content[0] = AssistantContent::Text(TextContent {
            text: "replacement".into(),
            text_signature: Some("late".into()),
        });
        a.content[1] = AssistantContent::Thinking(ThinkingContent {
            thinking: "redacted".into(),
            thinking_signature: Some("late thinking".into()),
            redacted: Some(true),
        });
        a.usage.input = -0.5;
        a.response_id = Some("response".into());
        a.diagnostics = Some(vec![]);
    }
    call.write()
        .unwrap()
        .arguments
        .insert("partial".into(), serde_json::Value::Null);
    call.write().unwrap().thought_signature = Some("late thought".into());
    s.push(AssistantMessageEvent::Done {
        reason: StopReason::Stop,
        message: a.clone(),
    })
    .unwrap();
    let mut i = s.iter();
    for _ in 0..2 {
        let event = ready(&mut i.next()).unwrap();
        let partial = match event {
            AssistantMessageEvent::Start { partial }
            | AssistantMessageEvent::TextDelta { partial, .. } => partial,
            _ => panic!(),
        };
        assert!(Arc::ptr_eq(&partial, &a));
        assert_eq!(
            partial.read().unwrap().response_id.as_deref(),
            Some("response")
        );
    }
    let AssistantMessageEvent::ToolcallEnd { tool_call, .. } = ready(&mut i.next()).unwrap() else {
        panic!()
    };
    assert!(Arc::ptr_eq(&tool_call, &call));
    assert_eq!(
        tool_call.read().unwrap().thought_signature.as_deref(),
        Some("late thought")
    );
    assert!(Arc::ptr_eq(&ready(&mut s.result()), &a));
}

#[test]
fn observation_drops_do_not_abort_producer() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    let (release, gate) = std::sync::mpsc::channel();
    let gate = Arc::new(Mutex::new(Some(gate)));
    let retained = Arc::new(Mutex::new(None));
    let observer = retained.clone();
    let worker = Arc::new(Mutex::new(None));
    let join = worker.clone();
    register_api_provider(
        ApiProvider {
            api: "producer".into(),
            stream: Arc::new(move |_, _, _| {
                let s = AssistantMessageEventStream::new();
                *observer.lock().unwrap() = Some(s.clone());
                let producer = s.clone();
                let gate = gate.lock().unwrap().take().unwrap();
                *join.lock().unwrap() = Some(std::thread::spawn(move || {
                    gate.recv().unwrap();
                    producer
                        .push(AssistantMessageEvent::Done {
                            reason: StopReason::Stop,
                            message: assistant(),
                        })
                        .unwrap();
                }));
                Ok(s)
            }),
            stream_simple: provider("unused").stream_simple,
        },
        None,
    );
    let s = stream(model("producer"), context(), None).unwrap();
    let mut cursor = s.iter();
    drop(cursor.next());
    drop(s.result());
    drop(s);
    drop(cursor);
    let retained = retained.lock().unwrap().take().unwrap();
    let mut remaining = retained.iter();
    let mut next = remaining.next();
    release.send(()).unwrap();
    worker.lock().unwrap().take().unwrap().join().unwrap();
    assert!(poll(&mut next).is_pending()); // The abandoned read received the terminal delivery.
    assert!(matches!(
        poll(&mut retained.result()),
        std::task::Poll::Ready(_)
    ));
    retained.end(None);
    assert!(ready(&mut next).is_none());
    clear_api_providers();
}

#[test]
fn generic_callback_failures_keep_ordered_state() {
    let order = Arc::new(Mutex::new(vec![]));
    let p = order.clone();
    let e = order.clone();
    let s = EventStream::new(
        Arc::new(move |v: &i32| {
            p.lock().unwrap().push("predicate");
            if *v == 1 {
                Err(ThrownValue::Number(1.0))
            } else {
                Ok(*v == 2)
            }
        }),
        Arc::new(move |_: &i32| -> Result<i32, ThrownValue> {
            e.lock().unwrap().push("extract");
            Err(ThrownValue::Number(2.0))
        }),
    );
    assert!(matches!(s.push(1), Err(ThrownValue::Number(1.0))));
    s.push(0).unwrap();
    assert!(matches!(s.push(2), Err(ThrownValue::Number(2.0))));
    s.push(3).unwrap();
    assert_eq!(
        *order.lock().unwrap(),
        ["predicate", "predicate", "predicate", "extract"]
    );
    let mut iter = s.iter();
    assert_eq!(ready(&mut iter.next()), Some(0));
    assert_eq!(ready(&mut iter.next()), None);
    assert!(poll(&mut s.result()).is_pending());
    s.end(Some(9));
    assert_eq!(ready(&mut s.result()), 9);
}

#[test]
fn terminal_push_leaves_other_waiters_until_end() {
    let s = queue();
    let mut a = s.iter();
    let mut b = s.iter();
    let mut first = a.next();
    let mut second = b.next();
    s.push(-1).unwrap();
    assert_eq!(ready(&mut first), Some(-1));
    assert!(poll(&mut second).is_pending());
    assert_eq!(ready(&mut s.result()), -1);
    s.end(None);
    assert_eq!(ready(&mut second), None);
}

#[test]
fn records_round_trip_all_wire_shapes() {
    for wire in [
        serde_json::json!({"v":1,"id":"message"}),
        serde_json::json!({"v":1,"id":"message","phase":"commentary"}),
        serde_json::json!({"v":1,"id":"message","phase":"final_answer"}),
    ] {
        let signature: TextSignatureV1 = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(signature.v, 1);
        assert_eq!(signature.id, "message");
        assert_eq!(serde_json::to_value(signature).unwrap(), wire);
    }
    for literal in [
        "openai-completions",
        "mistral-conversations",
        "openai-responses",
        "azure-openai-responses",
        "openai-codex-responses",
        "anthropic-messages",
        "bedrock-converse-stream",
        "google-generative-ai",
        "google-vertex",
    ] {
        let v: KnownApi = serde_json::from_value(serde_json::json!(literal)).unwrap();
        assert_eq!(serde_json::to_value(v).unwrap(), literal);
    }
    for literal in [
        "amazon-bedrock",
        "anthropic",
        "google",
        "google-vertex",
        "openai",
        "azure-openai-responses",
        "openai-codex",
        "deepseek",
        "github-copilot",
        "xai",
        "groq",
        "cerebras",
        "openrouter",
        "vercel-ai-gateway",
        "zai",
        "mistral",
        "minimax",
        "minimax-cn",
        "moonshotai",
        "moonshotai-cn",
        "huggingface",
        "fireworks",
        "opencode",
        "opencode-go",
        "kimi-coding",
        "cloudflare-workers-ai",
        "cloudflare-ai-gateway",
        "xiaomi",
        "xiaomi-token-plan-cn",
        "xiaomi-token-plan-ams",
        "xiaomi-token-plan-sgp",
    ] {
        let v: KnownProvider = serde_json::from_value(serde_json::json!(literal)).unwrap();
        assert_eq!(serde_json::to_value(v).unwrap(), literal);
    }
    for literal in ["minimal", "low", "medium", "high", "xhigh"] {
        let v: ThinkingLevel = serde_json::from_value(serde_json::json!(literal)).unwrap();
        assert_eq!(serde_json::to_value(v).unwrap(), literal);
    }
    for literal in ["off", "minimal", "low", "medium", "high", "xhigh"] {
        let v: ModelThinkingLevel = serde_json::from_value(serde_json::json!(literal)).unwrap();
        assert_eq!(serde_json::to_value(v).unwrap(), literal);
    }
    for literal in ["none", "short", "long"] {
        let v: CacheRetention = serde_json::from_value(serde_json::json!(literal)).unwrap();
        assert_eq!(serde_json::to_value(v).unwrap(), literal);
    }
    for literal in ["sse", "websocket", "websocket-cached", "auto"] {
        let v: Transport = serde_json::from_value(serde_json::json!(literal)).unwrap();
        assert_eq!(serde_json::to_value(v).unwrap(), literal);
    }
    let a = assistant();
    let partial = serde_json::to_value(a.read().unwrap().clone()).unwrap();
    let call = serde_json::json!({"type":"toolCall","id":"c","name":"tool","arguments":{},"thoughtSignature":"opaque"});
    for tag in [
        "start",
        "text_start",
        "text_delta",
        "text_end",
        "thinking_start",
        "thinking_delta",
        "thinking_end",
        "toolcall_start",
        "toolcall_delta",
        "toolcall_end",
        "done",
        "error",
    ] {
        let mut v = serde_json::json!({"type":tag});
        if tag == "done" {
            v["reason"] = serde_json::json!("stop");
            v["message"] = partial.clone();
        } else if tag == "error" {
            v["reason"] = serde_json::json!("error");
            v["error"] = partial.clone();
        } else {
            v["partial"] = partial.clone();
            if tag != "start" {
                v["contentIndex"] = serde_json::json!(-1.5);
            }
            if tag.ends_with("_delta") {
                v["delta"] = serde_json::json!("片");
            }
            if tag.ends_with("_end") && tag != "toolcall_end" {
                v["content"] = serde_json::json!("replacement");
            }
            if tag == "toolcall_end" {
                v["toolCall"] = call.clone();
            }
        }
        let event: AssistantMessageEvent = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(event).unwrap(), v, "{tag}");
    }
    for v in [
        serde_json::json!({"type":"text","text":"hello","textSignature":"opaque"}),
        serde_json::json!({"type":"thinking","thinking":"secret","thinkingSignature":"opaque","redacted":false}),
        call.clone(),
    ] {
        let c: AssistantContent = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(c).unwrap(), v);
    }
    for content in [
        serde_json::json!("plain"),
        serde_json::json!([{"type":"text","text":"block"},{"type":"image","data":"AA==","mimeType":"image/png"}]),
    ] {
        let v = serde_json::json!({"role":"user","content":content,"timestamp":-0.5});
        let u: UserMessage = serde_json::from_value(v.clone()).unwrap();
        assert_eq!(serde_json::to_value(u.clone()).unwrap(), v);
        assert_eq!(serde_json::to_value(Message::User(u)).unwrap(), v);
    }
    let tool: Tool = serde_json::from_value(
        serde_json::json!({"name":"t","description":"","parameters":{"type":"object"}}),
    )
    .unwrap();
    assert_eq!(tool.description, "");
    let mut keys = serde_json::Map::new();
    for key in ["10", "2", "01", "4294967295"] {
        keys.insert(key.into(), serde_json::Value::Null);
    }
    let mut nested = keys.clone();
    nested.insert("nested".into(), serde_json::Value::Object(keys.clone()));
    let supplied = ProviderStreamOptions {
        base: StreamOptions {
            metadata: Some(nested.clone()),
            headers: Some(keys.clone()),
            ..Default::default()
        },
        extra: nested.clone(),
    };
    let wire = serde_json::to_value(supplied).unwrap();
    for bag in [
        &wire["headers"],
        &wire["metadata"]["nested"],
        &wire["nested"],
    ] {
        assert_eq!(
            bag.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["2", "10", "01", "4294967295"]
        );
    }
    let call = ToolCall {
        id: "c".into(),
        name: "t".into(),
        arguments: nested,
        thought_signature: None,
    };
    let nested_wire = serde_json::to_value(call).unwrap();
    assert_eq!(
        nested_wire["arguments"]["nested"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["2", "10", "01", "4294967295"]
    );
    let mut descriptor = model("ordered");
    descriptor.headers = Some(keys.clone());
    descriptor.compat = Some(serde_json::json!({"nested":keys.clone()}));
    let descriptor_wire = serde_json::to_value(descriptor).unwrap();
    let tool_wire = serde_json::to_value(Tool {
        name: "t".into(),
        description: String::new(),
        parameters: serde_json::Value::Object(keys.clone()),
    })
    .unwrap();
    let result_wire = serde_json::to_value(ToolResultMessage {
        tool_call_id: "c".into(),
        tool_name: "t".into(),
        content: vec![],
        details: Some(serde_json::Value::Object(keys.clone())),
        is_error: false,
        timestamp: 0.0,
    })
    .unwrap();
    let response_wire = serde_json::to_value(ProviderResponse {
        status: 200.0,
        headers: keys.clone(),
    })
    .unwrap();
    let diagnostic_wire = serde_json::to_value(AssistantMessageDiagnostic {
        r#type: "custom".into(),
        timestamp: 0.0,
        error: None,
        details: Some(keys.clone()),
    })
    .unwrap();
    for bag in [
        &descriptor_wire["headers"],
        &descriptor_wire["compat"]["nested"],
        &tool_wire["parameters"],
        &result_wire["details"],
        &response_wire["headers"],
        &diagnostic_wire["details"],
    ] {
        assert_eq!(
            bag.as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["2", "10", "01", "4294967295"]
        );
    }
    let c = ToolCall {
        id: "c".into(),
        name: "n".into(),
        arguments: keys,
        thought_signature: None,
    };
    let wire = serde_json::to_value(c).unwrap();
    assert_eq!(
        wire["arguments"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["2", "10", "01", "4294967295"]
    );
}

#[test]
fn optional_null_and_numeric_values_are_not_normalized() {
    let absent = serde_json::to_value(context()).unwrap();
    assert!(absent.get("tools").is_none());
    let explicit = serde_json::to_value(Context {
        tools: Some(vec![]),
        ..context()
    })
    .unwrap();
    assert_eq!(explicit["tools"], serde_json::json!([]));
    for details in [None, Some(serde_json::Value::Null)] {
        let r = ToolResultMessage {
            tool_call_id: "c".into(),
            tool_name: "t".into(),
            content: vec![],
            details: details.clone(),
            is_error: false,
            timestamp: -1.25,
        };
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v.get("details"), details.as_ref());
        let decoded: ToolResultMessage = serde_json::from_value(v).unwrap();
        assert_eq!(decoded.details, details);
    }
    let a = assistant();
    {
        let mut a = a.write().unwrap();
        a.usage.input = -2.5;
        a.usage.total_tokens = 99.25;
        a.usage.cost.total = -100.5;
        a.diagnostics = Some(vec![]);
    }
    let v = serde_json::to_value(a.read().unwrap().clone()).unwrap();
    assert_eq!(v["usage"]["input"], -2.5);
    assert_eq!(v["usage"]["totalTokens"], 99.25);
    assert_eq!(v["usage"]["cost"]["total"], -100.5);
    assert!(v["usage"].get("reported").is_none());
    assert!(v["usage"]["cost"].get("priced").is_none());
    assert_eq!(v["diagnostics"], serde_json::json!([]));
    let mut m = model("unknown api");
    m.thinking_level_map = Some(
        serde_json::json!({"high":null})
            .as_object()
            .unwrap()
            .clone(),
    );
    assert!(m.thinking_level_map.as_ref().unwrap().get("low").is_none());
    assert_eq!(
        serde_json::to_value(&m).unwrap()["thinkingLevelMap"]["high"],
        serde_json::Value::Null
    );
    let mut extra = serde_json::Map::new();
    extra.insert("z".into(), serde_json::Value::Null);
    extra.insert("a".into(), serde_json::json!(false));
    let o = ProviderStreamOptions {
        base: StreamOptions {
            headers: Some(Default::default()),
            ..Default::default()
        },
        extra,
    };
    let v = serde_json::to_value(o).unwrap();
    assert_eq!(v["headers"], serde_json::json!({}));
    assert_eq!(v["z"], serde_json::Value::Null);
    assert_eq!(v["a"], false);
}

#[test]
fn compatibility_and_routing_fields_round_trip() {
    let completions = serde_json::json!({"supportsStore":false,"supportsDeveloperRole":true,"supportsReasoningEffort":false,"supportsUsageInStreaming":true,"maxTokensField":"max_tokens","requiresToolResultName":false,"requiresAssistantAfterToolResult":true,"requiresThinkingAsText":false,"requiresReasoningContentOnAssistantMessages":true,"thinkingFormat":"qwen-chat-template","openRouterRouting":{},"vercelGatewayRouting":{},"zaiToolStream":false,"supportsStrictMode":true,"cacheControlFormat":"anthropic","sendSessionAffinityHeaders":false,"supportsLongCacheRetention":true});
    let c: OpenAICompletionsCompat = serde_json::from_value(completions.clone()).unwrap();
    assert_eq!(serde_json::to_value(c).unwrap(), completions);
    let responses =
        serde_json::json!({"sendSessionIdHeader":false,"supportsLongCacheRetention":true});
    let r: OpenAIResponsesCompat = serde_json::from_value(responses.clone()).unwrap();
    assert_eq!(serde_json::to_value(r).unwrap(), responses);
    let messages = serde_json::json!({"supportsEagerToolInputStreaming":false,"supportsLongCacheRetention":true});
    let m: AnthropicMessagesCompat = serde_json::from_value(messages.clone()).unwrap();
    assert_eq!(serde_json::to_value(m).unwrap(), messages);
    for sort in [
        serde_json::json!("price"),
        serde_json::json!({"by":"throughput","partition":null}),
        serde_json::json!({"partition":"model"}),
    ] {
        let routing = serde_json::json!({"allow_fallbacks":false,"require_parameters":true,"data_collection":"deny","zdr":false,"enforce_distillable_text":true,"order":["a","b"],"only":[],"ignore":["x"],"quantizations":["fp8"],"sort":sort,"max_price":{"prompt":0.1,"completion":"0.2","image":0,"audio":"0","request":1},"preferred_min_throughput":{"p50":1,"p75":2,"p90":3,"p99":4},"preferred_max_latency":-0.5});
        let r: OpenRouterRouting = serde_json::from_value(routing.clone()).unwrap();
        assert_eq!(serde_json::to_value(r).unwrap(), routing);
    }
    for field_and_values in [
        (
            "maxTokensField",
            &["max_tokens", "max_completion_tokens"][..],
        ),
        (
            "thinkingFormat",
            &[
                "openai",
                "openrouter",
                "zai",
                "deepseek",
                "qwen",
                "qwen-chat-template",
            ][..],
        ),
        ("cacheControlFormat", &["anthropic"][..]),
    ] {
        for value in field_and_values.1 {
            let wire = serde_json::json!({field_and_values.0:*value});
            let typed: OpenAICompletionsCompat = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(typed).unwrap(), wire);
        }
    }
    for collection in ["allow", "deny"] {
        for by in ["price", "throughput", "latency"] {
            for partition in [
                serde_json::Value::Null,
                serde_json::json!("model"),
                serde_json::json!("none"),
            ] {
                let wire = serde_json::json!({"data_collection":collection,"sort":{"by":by,"partition":partition}});
                let typed: OpenRouterRouting = serde_json::from_value(wire.clone()).unwrap();
                assert_eq!(serde_json::to_value(typed).unwrap(), wire);
            }
        }
    }
    for field in ["preferred_min_throughput", "preferred_max_latency"] {
        for value in [
            serde_json::json!(0),
            serde_json::json!({"p50":0,"p75":-1.25,"p90":2,"p99":3}),
        ] {
            let wire = serde_json::json!({field:value});
            let typed: OpenRouterRouting = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(serde_json::to_value(typed).unwrap(), wire);
        }
    }
    let gateway = serde_json::json!({"only":[],"order":["a"]});
    let g: VercelGatewayRouting = serde_json::from_value(gateway.clone()).unwrap();
    assert_eq!(serde_json::to_value(g).unwrap(), gateway);
}

#[test]
fn raw_option_callbacks_and_extras_reach_adapter() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    let hooks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let observed = hooks.clone();
    let payload: StreamOptions = StreamOptions {
        on_payload: Some(Arc::new(move |v, _| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Box::pin(async move { Ok(Some(v)) })
        })),
        on_response: Some(Arc::new(|r, _| {
            assert_eq!(r.status, 200.0);
            Box::pin(async { Err(ThrownValue::Number(17.0)) })
        })),
        signal: Some(Cancellation::new()),
        ..Default::default()
    };
    let wire = serde_json::to_value(&payload).unwrap();
    assert_eq!(wire, serde_json::json!({"signal":{}}));
    let captured = Arc::new(Mutex::new(None));
    let target = captured.clone();
    register_api_provider(
        ApiProvider {
            api: "hooks".into(),
            stream: Arc::new(move |_, _, o| {
                *target.lock().unwrap() = o;
                Ok(AssistantMessageEventStream::new())
            }),
            stream_simple: provider("unused").stream_simple,
        },
        None,
    );
    stream(
        model("hooks"),
        context(),
        Some(ProviderStreamOptions {
            base: payload,
            extra: serde_json::json!({"arbitrary":null})
                .as_object()
                .unwrap()
                .clone(),
        }),
    )
    .unwrap();
    assert_eq!(hooks.load(std::sync::atomic::Ordering::SeqCst), 0);
    let o = captured.lock().unwrap().take().unwrap();
    for v in [
        serde_json::Value::Null,
        serde_json::json!({"replacement":true}),
    ] {
        let mut f = (o.base.on_payload.as_ref().unwrap())(v.clone(), model("hooks"));
        assert_eq!(ready(&mut f).unwrap(), Some(v));
    }
    let mut response = o.base.on_response.unwrap()(
        ProviderResponse {
            status: 200.0,
            headers: Default::default(),
        },
        model("hooks"),
    );
    assert!(matches!(
        ready(&mut response),
        Err(ThrownValue::Number(17.0))
    ));
    assert_eq!(o.extra["arbitrary"], serde_json::Value::Null);
    type HookWork = std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<Option<serde_json::Value>, ThrownValue>> + Send,
        >,
    >;
    for outcome in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!({"replace":true})),
    ] {
        let released = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let gate = released.clone();
        let hooks = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = hooks.clone();
        let expected = outcome.clone();
        let options = StreamOptions {
            on_payload: Some(Arc::new(move |payload, descriptor| {
                assert_eq!(payload, serde_json::json!({"original":true}));
                assert_eq!(descriptor.api, "controlled-hooks");
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let gate = gate.clone();
                let outcome = outcome.clone();
                Box::pin(async move {
                    std::future::poll_fn(|_| {
                        if gate.load(std::sync::atomic::Ordering::SeqCst) {
                            std::task::Poll::Ready(())
                        } else {
                            std::task::Poll::Pending
                        }
                    })
                    .await;
                    Ok(outcome)
                })
            })),
            ..Default::default()
        };
        let work: Arc<Mutex<Option<HookWork>>> = Arc::new(Mutex::new(None));
        let adapter_work = work.clone();
        let controlled = ApiProvider {
            api: "controlled-hooks".into(),
            stream: Arc::new(move |m, _, options| {
                let options = options.unwrap().base;
                *adapter_work.lock().unwrap() = Some(Box::pin(async move {
                    (options.on_payload.unwrap())(serde_json::json!({"original":true}), m).await
                }));
                Ok(create_assistant_message_event_stream())
            }),
            stream_simple: provider("unused").stream_simple,
        };
        register_api_provider(controlled, None);
        stream(
            model("controlled-hooks"),
            context(),
            Some(ProviderStreamOptions {
                base: options,
                ..Default::default()
            }),
        )
        .unwrap();
        assert_eq!(hooks.load(std::sync::atomic::Ordering::SeqCst), 0);
        let mut producer = work.lock().unwrap().take().unwrap();
        assert!(poll(&mut producer).is_pending());
        assert_eq!(hooks.load(std::sync::atomic::Ordering::SeqCst), 1);
        released.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(ready(&mut producer).unwrap(), expected);
    }
    for response_error in [false, true] {
        let gate = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let released = gate.clone();
        let callback: StreamOptions = StreamOptions {
            on_payload: Some(Arc::new(|_, _| {
                Box::pin(async { Err(ThrownValue::Number(-17.0)) })
            })),
            on_response: Some(Arc::new(move |_, _| {
                let gate = gate.clone();
                Box::pin(async move {
                    std::future::poll_fn(|_| {
                        if gate.load(std::sync::atomic::Ordering::SeqCst) {
                            std::task::Poll::Ready(())
                        } else {
                            std::task::Poll::Pending
                        }
                    })
                    .await;
                    if response_error {
                        Err(ThrownValue::Number(23.0))
                    } else {
                        Ok(())
                    }
                })
            })),
            ..Default::default()
        };
        let controlled: ApiStreamFunction = Arc::new(move |model, _, options| {
            assert!(options.unwrap().base.api_key.is_none());
            let mut payload =
                callback.on_payload.as_ref().unwrap()(serde_json::Value::Null, model.clone());
            assert!(matches!(
                ready(&mut payload),
                Err(ThrownValue::Number(-17.0))
            ));
            let mut response = callback.on_response.as_ref().unwrap()(
                ProviderResponse {
                    status: 204.0,
                    headers: Default::default(),
                },
                model,
            );
            assert!(poll(&mut response).is_pending());
            released.store(true, std::sync::atomic::Ordering::SeqCst);
            if response_error {
                assert!(matches!(
                    ready(&mut response),
                    Err(ThrownValue::Number(23.0))
                ));
            } else {
                ready(&mut response).unwrap();
            }
            Ok(create_assistant_message_event_stream())
        });
        controlled(
            model("hooks"),
            context(),
            Some(ProviderStreamOptions::default()),
        )
        .unwrap();
    }
    clear_api_providers();
}

#[test]
fn diagnostic_values_follow_string_coercion() {
    for (value, expected) in [
        (ThrownValue::Undefined, "undefined"),
        (ThrownValue::Json(serde_json::Value::Null), "null"),
        (ThrownValue::Json(serde_json::json!(true)), "true"),
        (ThrownValue::Json(serde_json::json!(false)), "false"),
        (ThrownValue::Json(serde_json::json!("")), ""),
        (ThrownValue::Json(serde_json::json!(" \n")), " \n"),
        (ThrownValue::Json(serde_json::json!({})), "[object Object]"),
        (
            ThrownValue::Json(serde_json::json!([null, ["x", null], {}, true])),
            ",x,,[object Object],true",
        ),
        (ThrownValue::Number(-0.0), "0"),
        (ThrownValue::Number(f64::NAN), "NaN"),
        (ThrownValue::Number(f64::INFINITY), "Infinity"),
        (ThrownValue::Number(f64::NEG_INFINITY), "-Infinity"),
        (ThrownValue::Number(1e-7), "1e-7"),
        (ThrownValue::Number(1e-6), "0.000001"),
        (ThrownValue::Number(1e20), "100000000000000000000"),
        (ThrownValue::Number(1e21), "1e+21"),
        (
            ThrownValue::Number(1000000000000000128.0),
            "1000000000000000100",
        ),
        (
            ThrownValue::StringCoercion(Arc::new(|| Ok("custom".into()))),
            "custom",
        ),
    ] {
        assert_eq!(format_thrown_value(&value).unwrap(), expected);
    }
    let custom = ThrownValue::StringCoercion(Arc::new(|| Err(ThrownValue::Number(0.0))));
    assert!(matches!(
        format_thrown_value(&custom),
        Err(ThrownValue::Number(0.0))
    ));
}

#[test]
fn diagnostic_errors_preserve_fields_and_code_kind() {
    for (code, expected) in [
        (None, None),
        (
            Some(ThrownValue::Json(serde_json::json!(""))),
            Some(DiagnosticCode::String(String::new())),
        ),
        (
            Some(ThrownValue::Number(0.0)),
            Some(DiagnosticCode::Number(0.0)),
        ),
        (Some(ThrownValue::Json(serde_json::json!(false))), None),
        (Some(ThrownValue::Json(serde_json::Value::Null)), None),
        (Some(ThrownValue::Undefined), None),
        (Some(ThrownValue::Json(serde_json::json!({}))), None),
    ] {
        let v = ThrownValue::Error(Box::new(Error {
            name: "Named".into(),
            message: "message".into(),
            stack: Some(String::new()),
            code,
        }));
        assert_eq!(
            extract_diagnostic_error(&v).unwrap(),
            DiagnosticErrorInfo {
                name: Some("Named".into()),
                message: "message".into(),
                stack: Some(String::new()),
                code: expected
            }
        );
    }
    let empty = ThrownValue::Error(Box::new(Error {
        name: String::new(),
        message: String::new(),
        stack: None,
        code: Some(ThrownValue::Number(f64::INFINITY)),
    }));
    let info = extract_diagnostic_error(&empty).unwrap();
    assert_eq!(info.name, None);
    assert_eq!(info.message, "");
    assert_eq!(info.code, Some(DiagnosticCode::Number(f64::INFINITY)));
    assert_eq!(
        extract_diagnostic_error(&ThrownValue::Undefined).unwrap(),
        DiagnosticErrorInfo {
            name: Some("ThrownValue".into()),
            message: "undefined".into(),
            stack: None,
            code: None
        }
    );
}

#[test]
fn diagnostic_creation_appends_without_rewriting() {
    let before = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    let details = serde_json::json!({"private":null})
        .as_object()
        .unwrap()
        .clone();
    let d = create_assistant_message_diagnostic(
        "kind".into(),
        &ThrownValue::Undefined,
        Some(details.clone()),
    )
    .unwrap();
    let after = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64;
    assert!(d.timestamp >= before && d.timestamp <= after);
    assert_eq!(d.details, Some(details));
    assert_eq!(d.r#type, "kind");
    let mut diagnostics = None;
    append_assistant_message_diagnostic(&mut diagnostics, d.clone());
    append_assistant_message_diagnostic(&mut diagnostics, d.clone());
    assert_eq!(diagnostics.unwrap(), [d.clone(), d]);
}

#[test]
fn cleanup_callbacks_use_identity_and_live_order() {
    let _guard = REGISTRY.lock().unwrap();
    let seen = Arc::new(Mutex::new(vec![]));
    let s = seen.clone();
    let callback: SessionResourceCleanup = Arc::new(move |id| {
        s.lock().unwrap().push(id.map(str::to_owned));
        Ok(())
    });
    let remove1 = register_session_resource_cleanup(callback.clone());
    let remove2 = register_session_resource_cleanup(callback);
    for id in [None, Some(""), Some("session")] {
        cleanup_session_resources(id).unwrap();
    }
    assert_eq!(
        *seen.lock().unwrap(),
        [None, Some(String::new()), Some("session".into())]
    );
    remove1();
    remove1();
    remove2();
    let order = Arc::new(Mutex::new(vec![]));
    let remove_unvisited = Arc::new(Mutex::new(None::<Box<dyn Fn() + Send + Sync>>));
    let added = Arc::new(Mutex::new(None::<Box<dyn Fn() + Send + Sync>>));
    let o = order.clone();
    let r = remove_unvisited.clone();
    let a = added.clone();
    let remove_first = register_session_resource_cleanup(Arc::new(move |_| {
        o.lock().unwrap().push("first");
        r.lock().unwrap().as_ref().unwrap()();
        let o = o.clone();
        *a.lock().unwrap() = Some(register_session_resource_cleanup(Arc::new(move |_| {
            o.lock().unwrap().push("added");
            Ok(())
        })));
        Ok(())
    }));
    let o = order.clone();
    *remove_unvisited.lock().unwrap() =
        Some(register_session_resource_cleanup(Arc::new(move |_| {
            o.lock().unwrap().push("removed");
            Ok(())
        })));
    cleanup_session_resources(None).unwrap();
    assert_eq!(*order.lock().unwrap(), ["first", "added"]);
    remove_first();
    added.lock().unwrap().as_ref().unwrap()();
}

#[test]
fn cleanup_aggregates_ordered_errors_after_sweep() {
    let _guard = REGISTRY.lock().unwrap();
    cleanup_session_resources(None).unwrap();
    let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let first = register_session_resource_cleanup(Arc::new(|_| Err(ThrownValue::Number(1.0))));
    let c = count.clone();
    let middle = register_session_resource_cleanup(Arc::new(move |_| {
        c.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }));
    let last = register_session_resource_cleanup(Arc::new(|_| Err(ThrownValue::Undefined)));
    for _ in 0..2 {
        let error = cleanup_session_resources(Some("")).unwrap_err();
        assert_eq!(error.to_string(), "Failed to cleanup session resources");
        assert!(matches!(
            error.errors.as_slice(),
            [ThrownValue::Number(1.0), ThrownValue::Undefined]
        ));
    }
    assert_eq!(count.load(std::sync::atomic::Ordering::SeqCst), 2);
    first();
    middle();
    last();
    cleanup_session_resources(None).unwrap();
}

#[test]
fn utf16_hash_matches_boundary_vectors() {
    for (units, expected) in [
        (&[][..], "k4n83c7h0j2b"),
        (&[0x61, 0x62, 0x63][..], "y0biex7f9bbh"),
        (&[0xe9][..], "1ohi2dgmq6lzp"),
        (&[0xfeff][..], "1ot1akt19m0c6r"),
        (&[0x85][..], "b78s7412emlzt"),
        (&[0xd83d, 0xde00][..], "13wj7r7usi372"),
        (&[0xd800][..], "sjkthq29gslh"),
        (&[0xdc00][..], "1czx0omv4xxbk"),
        (&[0x61, 0xd800, 0x62, 0xdc00][..], "frvkd01frvf3u"),
    ] {
        assert_eq!(short_hash(units), expected);
    }
}

#[test]
fn header_entries_become_last_assignment_record() {
    assert!(headers_to_record(Vec::<(String, String)>::new()).is_empty());
    let entries = [
        ("accept", "a, b"),
        ("set-cookie", "first=1"),
        ("x-value", " unchanged "),
        ("set-cookie", "last=2"),
    ]
    .map(|(k, v)| (k.into(), v.into()));
    let actual = headers_to_record(entries);
    assert_eq!(actual["accept"], "a, b");
    assert_eq!(actual["x-value"], " unchanged ");
    assert_eq!(actual["set-cookie"], "last=2");
    assert_eq!(
        actual.keys().map(String::as_str).collect::<Vec<_>>(),
        ["accept", "set-cookie", "x-value"]
    );
}

#[test]
fn string_enum_omits_falsy_options() {
    let values = vec![String::new(), "a".into(), "a".into()];
    assert_eq!(
        string_enum(&values, None, Some("")),
        serde_json::json!({"type":"string","enum":["","a","a"]})
    );
    assert_eq!(
        string_enum(&[], Some(""), None),
        serde_json::json!({"type":"string","enum":[]})
    );
    for whitespace in [" ", "\u{feff}", "\u{85}"] {
        assert_eq!(
            string_enum(&values, Some(whitespace), Some("not in values")),
            serde_json::json!({"type":"string","enum":["","a","a"],"description":whitespace,"default":"not in values"})
        );
    }
}

#[test]
fn public_surface_exports_records_and_helpers() {
    use maestro_models::records::{
        api_registry, diagnostics, event_stream, hash, headers, session_resources,
        stream as invocation, typebox_helpers, types,
    };
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    for callback in [
        Arc::new(|_, _, _| Ok(event_stream::create_assistant_message_event_stream()))
            as ApiStreamFunction,
        provider("swap").stream,
    ] {
        api_registry::register_api_provider(
            ApiProvider {
                api: "swap".into(),
                stream: callback,
                stream_simple: provider("swap").stream_simple,
            },
            None,
        );
        invocation::stream(model("swap"), context(), None).unwrap();
    }
    let _: types::Api = "open".into();
    let _: types::Provider = "custom".into();
    assert_eq!(hash::short_hash(&[]), short_hash(&[]));
    assert!(headers::headers_to_record([]).is_empty());
    assert_eq!(
        typebox_helpers::string_enum(&[], None, None),
        string_enum(&[], None, None)
    );
    session_resources::cleanup_session_resources(None).unwrap();
    assert_eq!(
        diagnostics::format_thrown_value(&ThrownValue::Undefined).unwrap(),
        "undefined"
    );
    let generic =
        event_stream::EventStream::<i32>::new(Arc::new(|_| Ok(false)), Arc::new(|v| Ok(*v)));
    generic.end(Some(0));
    assert_eq!(ready(&mut generic.result()), 0);
    clear_api_providers();
}

#[test]
fn observable_errors_match_exact_text() {
    let _guard = REGISTRY.lock().unwrap();
    clear_api_providers();
    for api in ["", "协议", "api\n"] {
        assert_eq!(
            text_error(stream(model(api), context(), None).err().unwrap()),
            format!("No API provider registered for api: {api}")
        );
    }
    register_api_provider(provider("right"), None);
    let p = get_api_provider("right").unwrap();
    assert_eq!(
        text_error((p.stream)(model("wrong"), context(), None).err().unwrap()),
        "Mismatched api: wrong expected right"
    );
    assert_eq!(
        text_error(
            (p.stream_simple)(model("wrong"), context(), None)
                .err()
                .unwrap()
        ),
        "Mismatched api: wrong expected right"
    );
    assert_eq!(
        extract_diagnostic_error(&ThrownValue::Json(serde_json::json!("raw custom text")))
            .unwrap()
            .name
            .as_deref(),
        Some("ThrownValue")
    );
    assert_eq!(
        format_thrown_value(&ThrownValue::Json(serde_json::json!("raw custom text"))).unwrap(),
        "raw custom text"
    );
    assert_eq!(
        AggregateError { errors: vec![] }.to_string(),
        "Failed to cleanup session resources"
    );
    clear_api_providers();
}

#[test]
fn docs_record_contract_paragraphs_match() {
    let docs = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../docs/records.md"
    ))
    .unwrap();
    for paragraph in [
        "A Model is a caller-supplied descriptor. Register an ApiProvider for its api, then call stream or stream_simple. Invocation does not require a model catalog entry.",
        "The raw and simple entry points forward their options to different adapter callbacks. This layer does not select authentication, project messages, merge headers, or apply provider defaults.",
        "AssistantMessageEventStream keeps queued events and one independently observable result. A terminal event settles result observation without consuming queued events. Earlier event handles observe later mutations of their shared message.",
        "Dropping an iterator, result future, or observing handle does not cancel producer-owned work. Calling end without a result closes iteration but leaves result observation pending; a later explicit result can settle it.",
        "Unknown APIs and adapter setup errors are invocation failures. An error event instead resolves the stream result to its error assistant message. Session resource cleanup runs the live callback set and reports all collected errors after the sweep.",
    ] {
        assert!(docs.contains(paragraph), "{paragraph}");
    }
}

#[test]
fn event_clone_can_end_its_stream_without_deadlock() {
    struct Reentrant(Arc<Mutex<Option<EventStream<Reentrant, i32>>>>);
    impl Clone for Reentrant {
        fn clone(&self) -> Self {
            self.0.lock().unwrap().as_ref().unwrap().end(None);
            Self(self.0.clone())
        }
    }
    let (sent, received) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let owner = Arc::new(Mutex::new(None));
        let stream = EventStream::new(Arc::new(|_| Ok(false)), Arc::new(|_| Ok(0)));
        *owner.lock().unwrap() = Some(stream.clone());
        stream.push(Reentrant(owner.clone())).unwrap();
        let mut cursor = stream.iter();
        assert!(matches!(
            poll(&mut cursor.next()),
            std::task::Poll::Ready(Some(_))
        ));
        assert!(matches!(
            poll(&mut cursor.next()),
            std::task::Poll::Ready(None)
        ));
        owner.lock().unwrap().take();
        sent.send(()).unwrap();
    });
    received
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("event Clone must run outside stream locks");
    worker.join().unwrap();
}

#[test]
fn header_records_enumerate_indices_before_strings() {
    let entries = [
        ("tail", "a"),
        ("10", "ten"),
        ("2", "two"),
        ("01", "leading"),
        ("4294967295", "not-index"),
        ("0", "zero"),
        ("tail", "updated"),
    ];
    let record = headers_to_record(entries.map(|(key, value)| (key.into(), value.into())));
    assert_eq!(
        record.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["0", "2", "10", "tail", "01", "4294967295"]
    );
    assert_eq!(record["tail"], "updated");
}

#[test]
fn header_proto_assignment_creates_no_own_property() {
    let record = headers_to_record([
        ("__proto__".into(), "ignored".into()),
        ("ok".into(), "kept".into()),
    ]);
    assert!(!record.contains_key("__proto__"));
    assert_eq!(record.len(), 1);
    assert_eq!(record["ok"], "kept");
}

#[test]
fn diagnostic_json_coercion_propagates_noncallable_to_string() {
    for value in [
        serde_json::json!({"toString":null}),
        serde_json::json!([{"toString":null}]),
        serde_json::json!([[{"toString":0}]]),
    ] {
        let thrown = ThrownValue::Json(value);
        let failure = create_assistant_message_diagnostic("kind".into(), &thrown, None)
            .expect_err("noncallable toString shadows object conversion");
        let ThrownValue::Error(error) = failure else {
            panic!("coercion must throw an error instance")
        };
        assert_eq!(error.name, "TypeError");
        assert_eq!(error.message, "Cannot convert object to primitive value");
    }
}

#[test]
fn flattened_raw_options_enumerate_root_integer_keys_before_base() {
    let mut extra = serde_json::Map::new();
    for key in ["z", "10", "2", "01", "a", "4294967295"] {
        extra.insert(key.into(), serde_json::json!(key));
    }
    let options = ProviderStreamOptions {
        base: StreamOptions {
            temperature: Some(0.25),
            ..Default::default()
        },
        extra,
    };
    let value = serde_json::to_value(&options).unwrap();
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["2", "10", "temperature", "z", "01", "a", "4294967295"]
    );
    assert!(
        serde_json::to_string(&options)
            .unwrap()
            .starts_with("{\"2\":\"2\",\"10\":\"10\",\"temperature\":0.25,")
    );
}

#[test]
fn nullable_routing_and_compat_distinguish_missing_from_explicit_null() {
    for field in ["sort", "preferred_min_throughput", "preferred_max_latency"] {
        let missing: OpenRouterRouting = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(
            !serde_json::to_value(&missing)
                .unwrap()
                .as_object()
                .unwrap()
                .contains_key(field)
        );
        let wire = serde_json::json!({field: null});
        let present: OpenRouterRouting = serde_json::from_value(wire.clone()).unwrap();
        let value = match field {
            "sort" => &present.sort,
            "preferred_min_throughput" => &present.preferred_min_throughput,
            _ => &present.preferred_max_latency,
        };
        assert_eq!(value, &Some(serde_json::Value::Null), "{field}");
        assert_eq!(serde_json::to_value(&present).unwrap(), wire);
    }
    let missing = model("custom");
    assert!(missing.compat.is_none());
    assert!(
        !serde_json::to_value(&missing)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("compat")
    );
    let mut wire = serde_json::to_value(missing).unwrap();
    wire["compat"] = serde_json::Value::Null;
    let present: Model = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(present.compat, Some(serde_json::Value::Null));
    assert_eq!(serde_json::to_value(present).unwrap(), wire);
}

#[test]
fn reentrant_serializer_can_write_shared_tool_call() {
    struct Reentrant(Arc<std::sync::RwLock<ToolCall>>);
    impl serde::Serializer for Reentrant {
        type Ok = ();
        type Error = serde_json::Error;
        type SerializeSeq = serde::ser::Impossible<(), serde_json::Error>;
        type SerializeTuple = serde::ser::Impossible<(), serde_json::Error>;
        type SerializeTupleStruct = serde::ser::Impossible<(), serde_json::Error>;
        type SerializeTupleVariant = serde::ser::Impossible<(), serde_json::Error>;
        type SerializeMap = serde::ser::Impossible<(), serde_json::Error>;
        type SerializeStruct = serde::ser::Impossible<(), serde_json::Error>;
        type SerializeStructVariant = serde::ser::Impossible<(), serde_json::Error>;
        fn serialize_struct(
            self,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStruct, Self::Error> {
            self.0
                .try_write()
                .expect("serializer must run without a record lock")
                .arguments["x"] = serde_json::json!(2);
            Err(<serde_json::Error as serde::ser::Error>::custom("observed"))
        }
        fn serialize_bool(self, _: bool) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_i8(self, _: i8) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_i16(self, _: i16) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_i32(self, _: i32) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_i64(self, _: i64) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_u8(self, _: u8) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_u16(self, _: u16) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_u32(self, _: u32) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_u64(self, _: u64) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_f32(self, _: f32) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_f64(self, _: f64) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_char(self, _: char) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_str(self, _: &str) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_bytes(self, _: &[u8]) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_none(self) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_unit(self) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_unit_struct(self, _: &'static str) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_unit_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
        ) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
            unreachable!()
        }
        fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
            unreachable!()
        }
        fn serialize_tuple_struct(
            self,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleStruct, Self::Error> {
            unreachable!()
        }
        fn serialize_tuple_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeTupleVariant, Self::Error> {
            unreachable!()
        }
        fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
            unreachable!()
        }
        fn serialize_struct_variant(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: usize,
        ) -> Result<Self::SerializeStructVariant, Self::Error> {
            unreachable!()
        }
        fn serialize_some<T: ?Sized + serde::Serialize>(self, _: &T) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_newtype_struct<T: ?Sized + serde::Serialize>(
            self,
            _: &'static str,
            _: &T,
        ) -> Result<(), Self::Error> {
            unreachable!()
        }
        fn serialize_newtype_variant<T: ?Sized + serde::Serialize>(
            self,
            _: &'static str,
            _: u32,
            _: &'static str,
            _: &T,
        ) -> Result<(), Self::Error> {
            unreachable!()
        }
    }
    let call = Arc::new(std::sync::RwLock::new(ToolCall {
        id: "call".into(),
        name: "lookup".into(),
        arguments: serde_json::json!({"x":1}).as_object().unwrap().clone(),
        thought_signature: None,
    }));
    let content = AssistantContent::ToolCall(call.clone());
    let error = serde::Serialize::serialize(&content, Reentrant(call.clone())).unwrap_err();
    assert_eq!(error.to_string(), "observed");
    assert_eq!(call.read().unwrap().arguments["x"], 2);
}
