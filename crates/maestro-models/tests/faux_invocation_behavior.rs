use maestro_models::*;
use std::{
    future::Future,
    sync::Arc,
    task::{Context as TaskContext, Poll, Wake, Waker},
};

struct ThreadWake(std::thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn run<T>(future: impl Future<Output = T>) -> T {
    let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
    let mut cx = TaskContext::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}
fn context() -> Context {
    Context {
        system_prompt: None,
        messages: vec![Message::User(UserMessage {
            content: UserContent::Text("hi".into()),
            timestamp: 0.0,
        })],
        tools: None,
    }
}
fn message(text: &str) -> AssistantMessage {
    faux_assistant_message(
        FauxAssistantContent::Text(text.into()),
        FauxAssistantMessageOptions::default(),
    )
}
#[test]
fn faux_registration_estimates_usage() {
    let registration = register_faux_provider(RegisterFauxProviderOptions::default());
    registration.set_responses(vec![FauxResponseStep::Message(message("hello world"))]);
    let result = run(complete(
        registration.get_model(None).unwrap(),
        context(),
        None,
    ))
    .unwrap();
    let result = result.read().unwrap();
    assert_eq!(result.content, message("hello world").content);
    assert!(result.usage.input > 0.0);
    assert_eq!(result.usage.output, 3.0);
    assert_eq!(
        result.usage.total_tokens,
        result.usage.input + result.usage.output
    );
    assert_eq!(registration.state.read().unwrap().call_count, 1.0);
    registration.unregister();
}
#[test]
fn faux_builders_form_mixed_content() {
    let registration = register_faux_provider(RegisterFauxProviderOptions::default());
    let tool = faux_tool_call(
        "echo".into(),
        serde_json::json!({"text":"hello"})
            .as_object()
            .unwrap()
            .clone(),
        FauxToolCallOptions::default(),
    );
    assert!(tool.id.starts_with("tool:"));
    let content = vec![
        AssistantContent::Thinking(faux_thinking("Thinking".into())),
        AssistantContent::ToolCall(Arc::new(std::sync::RwLock::new(tool))),
        AssistantContent::Text(faux_text("Done".into())),
    ];
    registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        FauxAssistantContent::Blocks(content.clone()),
        FauxAssistantMessageOptions {
            stop_reason: Some(StopReason::ToolUse),
            ..Default::default()
        },
    ))]);
    let result = run(complete(
        registration.get_model(None).unwrap(),
        context(),
        None,
    ))
    .unwrap();
    assert_eq!(result.read().unwrap().content, content);
    assert_eq!(result.read().unwrap().stop_reason, StopReason::ToolUse);
    registration.unregister();
}
fn definition(id: &str, reasoning: bool) -> FauxModelDefinition {
    FauxModelDefinition {
        id: id.into(),
        name: None,
        reasoning: Some(reasoning),
        input: None,
        cost: None,
        context_window: None,
        max_tokens: None,
    }
}
#[test]
fn faux_models_reach_response_factories() {
    let r = register_faux_provider(RegisterFauxProviderOptions {
        models: Some(vec![
            definition("fast", false),
            definition("thinking", true),
        ]),
        ..Default::default()
    });
    assert_eq!(r.get_model(None).unwrap().id, "fast");
    assert!(!r.get_model(Some("fast")).unwrap().reasoning);
    assert!(r.get_model(Some("thinking")).unwrap().reasoning);
    assert_eq!(
        r.models.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
        ["fast", "thinking"]
    );
    for (id, expected) in [("fast", "fast:false"), ("thinking", "thinking:true")] {
        r.append_responses(vec![FauxResponseStep::Factory(Arc::new(
            |_, _, _, model| {
                Box::pin(async move { Ok(message(&format!("{}:{}", model.id, model.reasoning))) })
            },
        ))]);
        let result = run(complete(r.get_model(Some(id)).unwrap(), context(), None)).unwrap();
        assert_eq!(
            result.read().unwrap().content,
            vec![AssistantContent::Text(faux_text(expected.into()))]
        );
    }
    r.unregister();
}
#[test]
fn faux_response_identity_follows_registration() {
    let r = register_faux_provider(RegisterFauxProviderOptions {
        provider: Some("custom".into()),
        ..Default::default()
    });
    r.set_responses(vec![FauxResponseStep::Message(message("identity"))]);
    let out = run(complete(r.get_model(None).unwrap(), context(), None)).unwrap();
    let out = out.read().unwrap();
    assert_eq!(out.api, r.api);
    assert_eq!(out.provider, "custom");
    assert_eq!(out.model, "faux-1");
    drop(out);
    let mut model = r.get_model(None).unwrap();
    model.provider = "requested-other".into();
    model.id = "requested-model".into();
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        |_, _, _, model| {
            assert_eq!(model.provider, "requested-other");
            Box::pin(async { Ok(message("identity")) })
        },
    ))]);
    let out = run(complete(model, context(), None)).unwrap();
    let out = out.read().unwrap();
    assert_eq!(out.api, r.api);
    assert_eq!(out.provider, "custom");
    assert_eq!(out.model, "requested-model");
    r.unregister();
}
#[test]
fn faux_queue_exhaustion_counts_requests() {
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![
        FauxResponseStep::Message(message("first")),
        FauxResponseStep::Message(message("second")),
    ]);
    for text in ["first", "second"] {
        let out = run(complete(r.get_model(None).unwrap(), context(), None)).unwrap();
        assert_eq!(
            out.read().unwrap().content,
            vec![AssistantContent::Text(faux_text(text.into()))]
        );
    }
    let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
    let mut iter = events.iter();
    let event = run(iter.next()).unwrap();
    assert!(matches!(
        event,
        AssistantMessageEvent::Error {
            reason: StopReason::Error,
            ..
        }
    ));
    assert!(run(iter.next()).is_none());
    assert_eq!(
        run(events.result())
            .read()
            .unwrap()
            .error_message
            .as_deref(),
        Some("No more faux responses queued")
    );
    assert_eq!(r.get_pending_response_count(), 0);
    assert_eq!(r.state.read().unwrap().call_count, 3.0);
    r.unregister();
}
#[test]
fn faux_queue_replacement_and_append() {
    let r = register_faux_provider(Default::default());
    assert_eq!(r.get_pending_response_count(), 0);
    r.set_responses(vec![FauxResponseStep::Message(message("old"))]);
    assert_eq!(r.get_pending_response_count(), 1);
    r.set_responses(vec![FauxResponseStep::Message(message("replacement"))]);
    assert_eq!(r.get_pending_response_count(), 1);
    r.append_responses(vec![
        FauxResponseStep::Message(message("third")),
        FauxResponseStep::Message(message("fourth")),
    ]);
    assert_eq!(r.get_pending_response_count(), 3);
    for (remaining, text) in [(2, "replacement"), (1, "third"), (0, "fourth")] {
        let out = run(complete(r.get_model(None).unwrap(), context(), None)).unwrap();
        assert_eq!(
            out.read().unwrap().content,
            vec![AssistantContent::Text(faux_text(text.into()))]
        );
        assert_eq!(r.get_pending_response_count(), remaining);
    }
    r.unregister();
}
#[test]
fn faux_async_factory_observes_context() {
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        |ctx, _, state, _| {
            Box::pin(async move {
                let count = state.read().unwrap().call_count;
                Ok(message(&format!("{}:{count}", ctx.messages.len())))
            })
        },
    ))]);
    let out = run(complete(r.get_model(None).unwrap(), context(), None)).unwrap();
    assert_eq!(
        out.read().unwrap().content,
        vec![AssistantContent::Text(faux_text("1:1".into()))]
    );
    r.unregister();
}
fn thrown(text: &str) -> ThrownValue {
    ThrownValue::Error(Box::new(Error {
        name: "Error".into(),
        message: text.into(),
        stack: None,
        code: None,
    }))
}
fn collect(events: &AssistantMessageEventStream) -> Vec<AssistantMessageEvent> {
    let mut iter = events.iter();
    let mut output = vec![];
    while let Some(event) = run(iter.next()) {
        output.push(event);
    }
    output
}
#[test]
fn faux_factory_failure_is_terminal() {
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(|_, _, _, _| {
        Box::pin(async { Err(thrown("boom")) })
    }))]);
    let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
    let output = collect(&events);
    assert_eq!(output.len(), 1);
    assert!(matches!(output[0], AssistantMessageEvent::Error { .. }));
    if let AssistantMessageEvent::Error { error, .. } = &output[0] {
        assert_eq!(error.read().unwrap().stop_reason, StopReason::Error);
    }
    assert_eq!(
        run(events.result())
            .read()
            .unwrap()
            .error_message
            .as_deref(),
        Some("boom")
    );
    r.unregister();
}
#[test]
fn faux_usage_serializes_all_context_roles() {
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![FauxResponseStep::Message(message("done"))]);
    let tool:Tool=serde_json::from_str(r#"{"name":"echo","description":"Echo back text","parameters":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"]}}"#).unwrap();
    let ctx = Context {
        system_prompt: Some("sys".into()),
        messages: vec![
            Message::User(UserMessage {
                content: UserContent::Blocks(vec![
                    InputContent::Text(faux_text("hello".into())),
                    InputContent::Image(ImageContent {
                        mime_type: "image/png".into(),
                        data: "abcd".into(),
                    }),
                ]),
                timestamp: 1.0,
            }),
            Message::Assistant(message("prior")),
            Message::ToolResult(ToolResultMessage {
                tool_call_id: "tool-1".into(),
                tool_name: "echo".into(),
                content: vec![InputContent::Text(faux_text("tool out".into()))],
                details: None,
                is_error: false,
                timestamp: 2.0,
            }),
        ],
        tools: Some(vec![tool]),
    };
    let out = run(complete(r.get_model(None).unwrap(), ctx, None)).unwrap();
    let usage = out.read().unwrap().usage.clone();
    // Literal serialization includes the tool schema in supplied property order.
    let prompt = "system:sys\n\nuser:hello\n[image:image/png:4]\n\nassistant:prior\n\ntoolResult:echo\ntool out\n\ntools:[{\"name\":\"echo\",\"description\":\"Echo back text\",\"parameters\":{\"type\":\"object\",\"properties\":{\"text\":{\"type\":\"string\"}},\"required\":[\"text\"]}}]";
    assert_eq!(
        usage.input,
        (prompt.encode_utf16().count() as f64 / 4.0).ceil()
    );
    assert_eq!(usage.output, 1.0);
    assert_eq!(usage.cache_read, 0.0);
    assert_eq!(usage.cache_write, 0.0);
    assert_eq!(usage.total_tokens, usage.input + 1.0);
    r.unregister();
}
fn options(
    session: Option<&str>,
    retention: Option<CacheRetention>,
) -> Option<ProviderStreamOptions> {
    Some(ProviderStreamOptions {
        base: StreamOptions {
            session_id: session.map(str::to_owned),
            cache_retention: retention,
            ..Default::default()
        },
        ..Default::default()
    })
}
#[test]
fn faux_cache_is_scoped_to_session() {
    let r = register_faux_provider(Default::default());
    for session in [Some("a"), Some("b"), None, None] {
        r.append_responses(vec![FauxResponseStep::Message(message("done"))]);
        let out = run(complete(
            r.get_model(None).unwrap(),
            context(),
            options(session, None),
        ))
        .unwrap();
        let usage = out.read().unwrap().usage.clone();
        assert_eq!(usage.cache_read, 0.0);
        assert_eq!(usage.cache_write, if session.is_some() { 2.0 } else { 0.0 });
    }
    r.unregister();
}
#[test]
fn faux_session_prefix_cache_is_reused() {
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![
        FauxResponseStep::Message(message("done")),
        FauxResponseStep::Message(message("done")),
    ]);
    let first = run(complete(
        r.get_model(None).unwrap(),
        context(),
        options(Some("session"), None),
    ))
    .unwrap();
    assert_eq!(first.read().unwrap().usage.cache_write, 2.0);
    let mut ctx = context();
    ctx.messages.push(Message::User(UserMessage {
        content: UserContent::Text("more".into()),
        timestamp: 0.0,
    }));
    let second = run(complete(
        r.get_model(None).unwrap(),
        ctx,
        options(Some("session"), None),
    ))
    .unwrap();
    assert_eq!(second.read().unwrap().usage.cache_read, 2.0);
    assert_eq!(second.read().unwrap().usage.cache_write, 3.0);
    r.unregister();
}
#[test]
fn faux_none_retention_skips_cache() {
    let r = register_faux_provider(Default::default());
    for _ in 0..2 {
        r.append_responses(vec![FauxResponseStep::Message(message("done"))]);
        let out = run(complete(
            r.get_model(None).unwrap(),
            context(),
            options(Some("session"), Some(CacheRetention::None)),
        ))
        .unwrap();
        let usage = out.read().unwrap().usage.clone();
        assert_eq!(usage.cache_read, 0.0);
        assert_eq!(usage.cache_write, 0.0);
    }
    r.unregister();
}
fn mixed_message() -> AssistantMessage {
    faux_assistant_message(
        FauxAssistantContent::Blocks(vec![
            AssistantContent::Thinking(faux_thinking("Thinking".into())),
            AssistantContent::Text(faux_text("Hello".into())),
            AssistantContent::ToolCall(Arc::new(std::sync::RwLock::new(faux_tool_call(
                "echo".into(),
                serde_json::json!({"text":"a long argument"})
                    .as_object()
                    .unwrap()
                    .clone(),
                FauxToolCallOptions {
                    id: Some("call-1".into()),
                },
            )))),
        ]),
        FauxAssistantMessageOptions {
            stop_reason: Some(StopReason::ToolUse),
            ..Default::default()
        },
    )
}
fn event_name(event: &AssistantMessageEvent) -> String {
    serde_json::to_value(event).unwrap()["type"]
        .as_str()
        .unwrap()
        .into()
}
#[test]
fn faux_mixed_blocks_emit_argument_chunks() {
    let r = register_faux_provider(RegisterFauxProviderOptions {
        token_size: Some(FauxTokenSize {
            min: Some(1.0),
            max: Some(1.0),
        }),
        ..Default::default()
    });
    r.set_responses(vec![FauxResponseStep::Message(mixed_message())]);
    let output = collect(&stream(r.get_model(None).unwrap(), context(), None).unwrap());
    for kind in [
        "thinking_start",
        "thinking_delta",
        "text_start",
        "text_delta",
        "toolcall_start",
        "toolcall_end",
    ] {
        assert!(
            output.iter().any(|e| event_name(e) == kind),
            "missing {kind}"
        );
    }
    let deltas = output
        .iter()
        .filter_map(|e| {
            if let AssistantMessageEvent::ToolcallDelta { delta, .. } = e {
                Some(delta.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    assert!(deltas.len() > 1);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&deltas.join("")).unwrap(),
        serde_json::json!({"text":"a long argument"})
    );
    r.unregister();
}
#[test]
fn faux_fixed_chunks_have_ordered_events() {
    let r = register_faux_provider(RegisterFauxProviderOptions {
        token_size: Some(FauxTokenSize {
            min: Some(1.0),
            max: Some(1.0),
        }),
        ..Default::default()
    });
    let mut msg = mixed_message();
    if let AssistantContent::Thinking(t) = &mut msg.content[0] {
        t.thinking = "a".into()
    }
    if let AssistantContent::Text(t) = &mut msg.content[1] {
        t.text = "b".into()
    }
    if let AssistantContent::ToolCall(t) = &msg.content[2] {
        t.write().unwrap().arguments.clear()
    }
    r.set_responses(vec![FauxResponseStep::Message(msg)]);
    let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
    let out = collect(&events);
    assert_eq!(
        out.iter().map(event_name).collect::<Vec<_>>(),
        [
            "start",
            "thinking_start",
            "thinking_delta",
            "thinking_end",
            "text_start",
            "text_delta",
            "text_end",
            "toolcall_start",
            "toolcall_delta",
            "toolcall_end",
            "done"
        ]
    );
    r.unregister();
}
#[test]
fn faux_multiple_tools_keep_content_indices() {
    let r = register_faux_provider(Default::default());
    let blocks = ["one", "two"].map(|id| {
        AssistantContent::ToolCall(Arc::new(std::sync::RwLock::new(faux_tool_call(
            "echo".into(),
            serde_json::Map::new(),
            FauxToolCallOptions {
                id: Some(id.into()),
            },
        ))))
    });
    r.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        FauxAssistantContent::Blocks(blocks.to_vec()),
        Default::default(),
    ))]);
    let out = collect(&stream(r.get_model(None).unwrap(), context(), None).unwrap());
    assert_eq!(
        out.iter()
            .filter_map(
                |e| if let AssistantMessageEvent::ToolcallStart { content_index, .. } = e {
                    Some(*content_index)
                } else {
                    None
                }
            )
            .collect::<Vec<_>>(),
        vec![0.0, 1.0]
    );
    assert_eq!(
        out.iter()
            .filter_map(|e| {
                if let AssistantMessageEvent::ToolcallEnd {
                    content_index,
                    tool_call,
                    ..
                } = e
                {
                    Some((*content_index, tool_call.read().unwrap().id.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>(),
        vec![(0.0, "one".into()), (1.0, "two".into())]
    );
    r.unregister();
}
#[test]
fn faux_explicit_error_follows_content() {
    let r = register_faux_provider(Default::default());
    let mut msg = message("partial");
    msg.stop_reason = StopReason::Error;
    msg.error_message = Some("upstream failed".into());
    r.set_responses(vec![FauxResponseStep::Message(msg)]);
    let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
    let out = collect(&events);
    assert_eq!(
        out.iter().map(event_name).collect::<Vec<_>>(),
        ["start", "text_start", "text_delta", "text_end", "error"]
    );
    assert!(matches!(
        out.last().unwrap(),
        AssistantMessageEvent::Error {
            reason: StopReason::Error,
            ..
        }
    ));
    assert_eq!(
        run(events.result())
            .read()
            .unwrap()
            .error_message
            .as_deref(),
        Some("upstream failed")
    );
    if let AssistantMessageEvent::Error { error, .. } = out.last().unwrap() {
        let terminal = error.read().unwrap();
        assert_eq!(terminal.stop_reason, StopReason::Error);
        assert_eq!(terminal.error_message.as_deref(), Some("upstream failed"));
    } else {
        panic!("expected terminal error")
    }
    r.unregister();
}
#[test]
fn faux_explicit_abort_follows_content() {
    let r = register_faux_provider(Default::default());
    let mut msg = message("partial");
    msg.stop_reason = StopReason::Aborted;
    msg.error_message = Some("Request was aborted".into());
    r.set_responses(vec![FauxResponseStep::Message(msg)]);
    let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
    let out = collect(&events);
    assert_eq!(
        out.iter().map(event_name).collect::<Vec<_>>(),
        ["start", "text_start", "text_delta", "text_end", "error"]
    );
    assert!(matches!(
        out.last().unwrap(),
        AssistantMessageEvent::Error {
            reason: StopReason::Aborted,
            ..
        }
    ));
    assert_eq!(
        run(events.result())
            .read()
            .unwrap()
            .error_message
            .as_deref(),
        Some("Request was aborted")
    );
    if let AssistantMessageEvent::Error { error, .. } = out.last().unwrap() {
        let terminal = error.read().unwrap();
        assert_eq!(terminal.stop_reason, StopReason::Aborted);
        assert_eq!(
            terminal.error_message.as_deref(),
            Some("Request was aborted")
        );
    } else {
        panic!("expected terminal error")
    }
    r.unregister();
}
#[test]
fn faux_preabort_consumes_queued_response() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let called = Arc::new(AtomicUsize::new(0));
    let hooks = called.clone();
    let factories = called.clone();
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        move |_, _, _, _| {
            factories.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(message("full output")) })
        },
    ))]);
    let signal = Cancellation::new();
    signal.cancel();
    let mut opts = options(Some("session"), None).unwrap();
    opts.base.signal = Some(signal);
    opts.base.on_response = Some(Arc::new(move |response, _| {
        assert_eq!(response.status, 200.0);
        assert!(response.headers.is_empty());
        hooks.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }));
    let events = stream(r.get_model(None).unwrap(), context(), Some(opts)).unwrap();
    let out = collect(&events);
    assert_eq!(out.len(), 1);
    assert!(matches!(
        out[0],
        AssistantMessageEvent::Error {
            reason: StopReason::Aborted,
            ..
        }
    ));
    assert_eq!(called.load(Ordering::SeqCst), 2);
    assert_eq!(r.get_pending_response_count(), 0);
    assert_eq!(r.state.read().unwrap().call_count, 1.0);
    r.append_responses(vec![FauxResponseStep::Message(message("ok"))]);
    let out = run(complete(
        r.get_model(None).unwrap(),
        context(),
        options(Some("session"), None),
    ))
    .unwrap();
    assert_eq!(out.read().unwrap().usage.cache_read, 2.0);
    r.unregister();
}
fn paced_abort(kind: &str, rate: f64) {
    let r = register_faux_provider(RegisterFauxProviderOptions {
        tokens_per_second: Some(rate),
        token_size: Some(FauxTokenSize {
            min: Some(3.0),
            max: Some(3.0),
        }),
        ..Default::default()
    });
    let text = "abcdefghijklmnopqrstuvwxyz";
    let block = match kind {
        "text" => AssistantContent::Text(faux_text(text.into())),
        "thinking" => AssistantContent::Thinking(faux_thinking(text.into())),
        _ => AssistantContent::ToolCall(Arc::new(std::sync::RwLock::new(faux_tool_call(
            "echo".into(),
            serde_json::json!({"text":text})
                .as_object()
                .unwrap()
                .clone(),
            FauxToolCallOptions {
                id: Some("tool-1".into()),
            },
        )))),
    };
    r.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
        FauxAssistantContent::Block(block),
        Default::default(),
    ))]);
    let signal = Cancellation::new();
    let opts = ProviderStreamOptions {
        base: StreamOptions {
            signal: Some(signal.clone()),
            ..Default::default()
        },
        ..Default::default()
    };
    let events = stream(r.get_model(None).unwrap(), context(), Some(opts)).unwrap();
    let mut iter = events.iter();
    let mut names = vec![];
    while let Some(event) = run(iter.next()) {
        let name = event_name(&event);
        if name == format!("{kind}_delta") {
            signal.cancel();
        }
        names.push(name);
    }
    assert_eq!(
        names
            .iter()
            .filter(|n| **n == format!("{kind}_delta"))
            .count(),
        1
    );
    assert!(!names.contains(&format!("{kind}_end")));
    let out = run(events.result());
    let out = out.read().unwrap();
    assert_eq!(out.stop_reason, StopReason::Aborted);
    assert_eq!(out.error_message.as_deref(), Some("Request was aborted"));
    match &out.content[0] {
        AssistantContent::Text(t) => assert_eq!(t.text, "abcdefghijkl"),
        AssistantContent::Thinking(t) => assert_eq!(t.thinking, "abcdefghijkl"),
        AssistantContent::ToolCall(t) => assert!(t.read().unwrap().arguments.is_empty()),
    }
    r.unregister();
}
#[test]
fn faux_paced_text_abort_keeps_prefix() {
    paced_abort("text", 50.0);
}
#[test]
fn faux_paced_thinking_abort_keeps_prefix() {
    paced_abort("thinking", 100.0);
}
#[test]
fn faux_paced_tool_abort_keeps_empty_arguments() {
    paced_abort("toolcall", 100.0);
}
#[test]
fn faux_unregister_removes_source() {
    let r = register_faux_provider(Default::default());
    let model = r.get_model(None).unwrap();
    r.unregister();
    let error = run(complete(model, context(), None)).unwrap_err();
    assert_eq!(
        format_thrown_value(&error).unwrap(),
        format!("No API provider registered for api: {}", r.api)
    );
}
#[test]
fn faux_defaults_and_model_overrides() {
    for models in [None, Some(vec![])] {
        let r = register_faux_provider(RegisterFauxProviderOptions {
            models,
            ..Default::default()
        });
        let m = r.get_model(None).unwrap();
        assert_eq!(
            (
                m.id.as_str(),
                m.name.as_str(),
                m.provider.as_str(),
                m.base_url.as_str()
            ),
            ("faux-1", "Faux Model", "faux", "http://localhost:0")
        );
        assert_eq!(m.input, ["text", "image"]);
        assert_eq!((m.context_window, m.max_tokens), (128000.0, 16384.0));
        assert!(!m.reasoning);
        assert_eq!(m.cost.input, 0.0);
        r.unregister();
    }
    let d = FauxModelDefinition {
        id: "".into(),
        name: Some("".into()),
        reasoning: Some(true),
        input: Some(vec![]),
        cost: Some(TokenRates {
            input: 1.0,
            output: 2.0,
            cache_read: 3.0,
            cache_write: 4.0,
        }),
        context_window: Some(-0.5),
        max_tokens: Some(0.0),
    };
    let r = register_faux_provider(RegisterFauxProviderOptions {
        api: Some("".into()),
        provider: Some("".into()),
        models: Some(vec![
            d.clone(),
            d,
            definition("same", false),
            definition("same", true),
        ]),
        ..Default::default()
    });
    let m = r.get_model(Some("")).unwrap();
    assert_eq!(m.api, "");
    assert_eq!(m.provider, "");
    assert_eq!(m.name, "");
    assert!(m.input.is_empty());
    assert_eq!((m.context_window, m.max_tokens), (-0.5, 0.0));
    assert_eq!(m.cost.cache_write, 4.0);
    assert!(r.get_model(Some("missing")).is_none());
    assert!(!r.get_model(Some("same")).unwrap().reasoning);
    r.unregister();
    let m = faux_assistant_message(
        FauxAssistantContent::Text("".into()),
        FauxAssistantMessageOptions {
            timestamp: Some(0.0),
            error_message: Some("".into()),
            response_id: Some("".into()),
            ..Default::default()
        },
    );
    assert_eq!(
        (m.api.as_str(), m.provider.as_str(), m.model.as_str()),
        ("faux", "faux", "faux-1")
    );
    assert_eq!(m.timestamp, 0.0);
    assert_eq!(m.error_message.as_deref(), Some(""));
    assert_eq!(m.response_id.as_deref(), Some(""));
    assert_eq!(m.usage.total_tokens, 0.0);
}
#[test]
fn faux_content_inputs_and_clone_isolation() {
    let r = register_faux_provider(Default::default());
    for (input, count) in [
        (FauxAssistantContent::Text("".into()), 1),
        (FauxAssistantContent::Text("hi".into()), 1),
        (
            FauxAssistantContent::Block(AssistantContent::Text(faux_text("a".into()))),
            1,
        ),
        (FauxAssistantContent::Blocks(vec![]), 0),
    ] {
        assert_eq!(
            faux_assistant_message(input, Default::default())
                .content
                .len(),
            count
        );
    }
    let mut msg = mixed_message();
    if let AssistantContent::Thinking(t) = &mut msg.content[0] {
        t.thinking_signature = Some("sig".into());
        t.redacted = Some(true)
    }
    if let AssistantContent::Text(t) = &mut msg.content[1] {
        t.text_signature = Some("text-sig".into())
    }
    msg.response_id = Some("response".into());
    msg.response_model = Some("actual".into());
    let AssistantContent::ToolCall(original) = &msg.content[2] else {
        unreachable!()
    };
    let original = original.clone();
    original.write().unwrap().thought_signature = Some("tool-sig".into());
    r.set_responses(vec![FauxResponseStep::Message(msg.clone())]);
    let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
    let out = collect(&events);
    let result = run(events.result());
    let terminal = match out.last().unwrap() {
        AssistantMessageEvent::Done { message, .. } => message,
        _ => panic!(),
    };
    assert!(Arc::ptr_eq(&result, terminal));
    let final_message = result.read().unwrap().clone();
    assert_eq!(final_message.content, msg.content);
    assert_eq!(final_message.response_model, msg.response_model);
    assert_eq!(final_message.response_id, msg.response_id);
    let AssistantContent::ToolCall(cloned) = &final_message.content[2] else {
        unreachable!()
    };
    assert!(!Arc::ptr_eq(&original, cloned));
    for event in &out {
        if let AssistantMessageEvent::ThinkingStart { partial, .. } = event {
            let p = partial.read().unwrap();
            let AssistantContent::Thinking(t) = &p.content[0] else {
                panic!()
            };
            assert_eq!(t.thinking, "");
            assert!(t.thinking_signature.is_none());
            assert!(t.redacted.is_none());
        }
    }
    for event in &out {
        if let AssistantMessageEvent::ToolcallEnd {
            partial, tool_call, ..
        } = event
        {
            assert_eq!(
                tool_call.read().unwrap().thought_signature.as_deref(),
                Some("tool-sig")
            );
            let partial = partial.read().unwrap();
            let AssistantContent::ToolCall(tool) = &partial.content[2] else {
                panic!()
            };
            assert!(tool.read().unwrap().thought_signature.is_none());
        }
    }
    r.unregister();
}
#[test]
fn faux_response_hook_precedes_factory() {
    use std::sync::Mutex;
    let order = Arc::new(Mutex::new(vec![]));
    let hook_order = order.clone();
    let factory_order = order.clone();
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        move |_, _, _, model| {
            assert_eq!(model.id, "faux-1");
            factory_order.lock().unwrap().push("factory");
            Box::pin(async { Ok(message("ok")) })
        },
    ))]);
    let opts = ProviderStreamOptions {
        base: StreamOptions {
            on_response: Some(Arc::new(move |response, model| {
                assert_eq!(response.status, 200.0);
                assert!(response.headers.is_empty());
                assert_eq!(model.id, "faux-1");
                let order = hook_order.clone();
                Box::pin(async move {
                    order.lock().unwrap().push("hook");
                    Ok(())
                })
            })),
            on_payload: Some(Arc::new(|_, _| panic!("payload invoked"))),
            ..Default::default()
        },
        ..Default::default()
    };
    run(complete(
        r.get_model(None).unwrap(),
        context(),
        Some(opts.clone()),
    ))
    .unwrap();
    assert_eq!(*order.lock().unwrap(), vec!["hook", "factory"]);
    let exhausted = run(complete(r.get_model(None).unwrap(), context(), Some(opts))).unwrap();
    assert_eq!(
        exhausted.read().unwrap().error_message.as_deref(),
        Some("No more faux responses queued")
    );
    assert_eq!(*order.lock().unwrap(), vec!["hook", "factory", "hook"]);
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(|_, _, _, _| {
        panic!("factory after failed hook")
    }))]);
    let opts = ProviderStreamOptions {
        base: StreamOptions {
            on_response: Some(Arc::new(|_, _| {
                Box::pin(async { Err(thrown("hook failed")) })
            })),
            session_id: Some("failure".into()),
            ..Default::default()
        },
        ..Default::default()
    };
    let events = stream(r.get_model(None).unwrap(), context(), Some(opts)).unwrap();
    assert_eq!(collect(&events).len(), 1);
    let out = run(events.result());
    assert_eq!(out.read().unwrap().usage.total_tokens, 0.0);
    assert_eq!(
        out.read().unwrap().error_message.as_deref(),
        Some("hook failed")
    );
    assert_eq!(r.state.read().unwrap().call_count, 3.0);
    r.unregister();
}
#[test]
fn faux_factory_receives_unmodified_options() {
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        |ctx, options, state, model| {
            assert_eq!(ctx.messages.len(), 1);
            assert!(options.is_none());
            assert_eq!(state.read().unwrap().call_count, 1.0);
            assert_eq!(model.id, "faux-1");
            Box::pin(async { Ok(message("absent")) })
        },
    ))]);
    run(complete(r.get_model(None).unwrap(), context(), None)).unwrap();
    let signal = Cancellation::new();
    let seen_signal = signal.clone();
    r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
        move |_, options, _, _| {
            let options = options.unwrap();
            assert!(options.base.on_response.is_some());
            assert!(options.base.on_payload.is_some());
            if options.extra.get("reasoning") != Some(&serde_json::json!("high")) {
                return Box::pin(async { Err(thrown("missing simple options")) });
            }
            assert_eq!(options.extra["thinkingBudgets"]["high"], 42.0);
            assert_eq!(options.base.temperature, Some(-0.5));
            assert!(options.base.api_key.is_none());
            options.base.signal.unwrap().cancel();
            assert!(seen_signal.is_cancelled());
            Box::pin(async { Ok(message("present")) })
        },
    ))]);
    let simple = SimpleStreamOptions {
        base: StreamOptions {
            signal: Some(signal),
            temperature: Some(-0.5),
            on_response: Some(Arc::new(|_, _| Box::pin(async { Ok(()) }))),
            on_payload: Some(Arc::new(|_, _| panic!())),
            ..Default::default()
        },
        reasoning: Some(ThinkingLevel::High),
        thinking_budgets: Some(ThinkingBudgets {
            minimal: None,
            low: None,
            medium: None,
            high: Some(42.0),
        }),
    };
    let out = run(complete_simple(
        r.get_model(None).unwrap(),
        context(),
        Some(simple),
    ))
    .unwrap();
    assert_eq!(out.read().unwrap().stop_reason, StopReason::Aborted);
    r.unregister();
}
#[test]
fn faux_errors_keep_string_coercion() {
    let r = register_faux_provider(Default::default());
    let cases = vec![
        (thrown(""), ""),
        (thrown("boom"), "boom"),
        (ThrownValue::Undefined, "undefined"),
        (ThrownValue::Json(serde_json::Value::Null), "null"),
        (ThrownValue::Json(serde_json::json!(true)), "true"),
        (ThrownValue::Number(f64::NAN), "NaN"),
        (
            ThrownValue::StringCoercion(Arc::new(|| Ok("custom".into()))),
            "custom",
        ),
    ];
    for (value, expected) in cases {
        r.set_responses(vec![FauxResponseStep::Factory(Arc::new(
            move |_, _, _, _| {
                let value = value.clone();
                Box::pin(async move { Err(value) })
            },
        ))]);
        let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
        assert_eq!(collect(&events).len(), 1);
        let out = run(events.result());
        assert_eq!(out.read().unwrap().error_message.as_deref(), Some(expected));
        assert_eq!(out.read().unwrap().usage.total_tokens, 0.0);
    }
    let out = run(complete(r.get_model(None).unwrap(), context(), None)).unwrap();
    assert_eq!(out.read().unwrap().usage.input, 2.0);
    r.unregister();
}
#[test]
fn faux_cache_prefix_rounding_and_persistence() {
    let r = register_faux_provider(RegisterFauxProviderOptions {
        models: Some(vec![FauxModelDefinition {
            cost: Some(TokenRates {
                input: 9.0,
                output: 9.0,
                cache_read: 9.0,
                cache_write: 9.0,
            }),
            ..definition("priced", false)
        }]),
        ..Default::default()
    });
    for (text, retention, expected) in [
        ("abc😀x", None, (3.0, 0.0, 3.0, 7.0)),
        ("abc😀xy", Some(CacheRetention::Short), (0.0, 3.0, 1.0, 5.0)),
        ("abc😀", Some(CacheRetention::Long), (0.0, 3.0, 0.0, 4.0)),
        ("abcZ", None, (1.0, 2.0, 1.0, 5.0)),
        ("no cache", Some(CacheRetention::None), (4.0, 0.0, 0.0, 5.0)),
        ("abcZ", None, (0.0, 3.0, 0.0, 4.0)),
    ] {
        r.set_responses(vec![FauxResponseStep::Message(message("done"))]);
        let ctx = Context {
            messages: vec![Message::User(UserMessage {
                content: UserContent::Text(text.into()),
                timestamp: 0.0,
            })],
            system_prompt: None,
            tools: None,
        };
        let out = run(complete(
            r.get_model(None).unwrap(),
            ctx,
            options(Some("cache"), retention),
        ))
        .unwrap();
        let usage = out.read().unwrap().usage.clone();
        assert_eq!(
            (
                usage.input,
                usage.cache_read,
                usage.cache_write,
                usage.total_tokens
            ),
            expected,
            "{text}"
        );
        assert_eq!(
            (
                usage.cost.input,
                usage.cost.output,
                usage.cost.cache_read,
                usage.cost.cache_write,
                usage.cost.total
            ),
            (0.0, 0.0, 0.0, 0.0, 0.0)
        );
    }
    for session in [None, Some("")] {
        r.set_responses(vec![FauxResponseStep::Message(message("done"))]);
        let out = run(complete(
            r.get_model(None).unwrap(),
            context(),
            options(session, None),
        ))
        .unwrap();
        assert_eq!(out.read().unwrap().usage.cache_write, 0.0);
    }
    r.unregister();
}
#[test]
fn faux_terminal_reason_and_final_message_are_preserved() {
    let r = register_faux_provider(Default::default());
    for reason in [
        StopReason::Stop,
        StopReason::Length,
        StopReason::ToolUse,
        StopReason::Error,
        StopReason::Aborted,
    ] {
        for content in [
            vec![],
            vec![
                AssistantContent::Text(faux_text("".into())),
                AssistantContent::Thinking(faux_thinking("".into())),
            ],
            mixed_message().content,
        ] {
            let msg = faux_assistant_message(
                FauxAssistantContent::Blocks(content.clone()),
                FauxAssistantMessageOptions {
                    stop_reason: Some(reason.clone()),
                    error_message: Some("explicit".into()),
                    timestamp: Some(42.0),
                    ..Default::default()
                },
            );
            r.set_responses(vec![FauxResponseStep::Message(msg)]);
            let events = stream(r.get_model(None).unwrap(), context(), None).unwrap();
            let out = collect(&events);
            let terminal = out.last().unwrap();
            assert_eq!(
                event_name(terminal),
                if matches!(reason, StopReason::Error | StopReason::Aborted) {
                    "error"
                } else {
                    "done"
                }
            );
            let final_message = run(events.result());
            let final_message = final_message.read().unwrap();
            assert_eq!(final_message.stop_reason, reason);
            assert_eq!(final_message.content, content);
            assert_eq!(final_message.timestamp, 42.0);
            assert_eq!(final_message.error_message.as_deref(), Some("explicit"));
        }
    }
    r.unregister();
}

#[test]
fn faux_supplied_usage_is_replaced() {
    for factory in [false, true] {
        for cache in [false, true] {
            let r = register_faux_provider(Default::default());
            let mut supplied = message("done");
            supplied.usage = Usage {
                input: 31.0,
                output: 37.0,
                cache_read: 7.0,
                cache_write: 9.0,
                total_tokens: 84.0,
                cost: UsageCost {
                    input: 1.0,
                    output: 2.0,
                    cache_read: 3.0,
                    cache_write: 4.0,
                    total: 10.0,
                },
            };
            let step = if factory {
                FauxResponseStep::Factory(Arc::new(move |_, _, _, _| {
                    let supplied = supplied.clone();
                    Box::pin(async move { Ok(supplied) })
                }))
            } else {
                FauxResponseStep::Message(supplied)
            };
            r.set_responses(vec![step.clone(), step]);
            for repeated in [false, true] {
                let opts = options(
                    Some("usage"),
                    Some(if cache {
                        CacheRetention::Short
                    } else {
                        CacheRetention::None
                    }),
                );
                let out = run(complete(r.get_model(None).unwrap(), context(), opts)).unwrap();
                let usage = out.read().unwrap().usage.clone();
                let expected = match (cache, repeated) {
                    (false, _) => (2.0, 1.0, 0.0, 0.0, 3.0),
                    (true, false) => (2.0, 1.0, 0.0, 2.0, 5.0),
                    (true, true) => (0.0, 1.0, 2.0, 0.0, 3.0),
                };
                assert_eq!(
                    (
                        usage.input,
                        usage.output,
                        usage.cache_read,
                        usage.cache_write,
                        usage.total_tokens
                    ),
                    expected
                );
                assert_eq!(
                    usage.cost,
                    UsageCost {
                        input: 0.0,
                        output: 0.0,
                        cache_read: 0.0,
                        cache_write: 0.0,
                        total: 0.0
                    }
                );
            }
            r.unregister();
        }
    }
}

#[test]
fn faux_reader_cancels_after_first_delta() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    runtime.block_on(async {
        let r = register_faux_provider(RegisterFauxProviderOptions {
            tokens_per_second: Some(1000.0),
            token_size: Some(FauxTokenSize {
                min: Some(1.0),
                max: Some(1.0),
            }),
            ..Default::default()
        });
        let text = "a".repeat(4096);
        r.set_responses(vec![FauxResponseStep::Message(message(&text))]);
        let signal = Cancellation::new();
        let opts = ProviderStreamOptions {
            base: StreamOptions {
                signal: Some(signal.clone()),
                ..Default::default()
            },
            ..Default::default()
        };
        let events = stream(r.get_model(None).unwrap(), context(), Some(opts)).unwrap();
        let mut iter = events.iter();
        while let Some(event) = iter.next().await {
            if matches!(event, AssistantMessageEvent::TextDelta { .. }) {
                signal.cancel();
                break;
            }
        }
        let result = events.result().await;
        let result = result.read().unwrap();
        assert_eq!(result.stop_reason, StopReason::Aborted);
        let AssistantContent::Text(content) = &result.content[0] else {
            panic!()
        };
        assert!(!content.text.is_empty());
        assert!(content.text.len() < text.len());
        r.unregister();
    });
}

#[test]
fn faux_registration_survives_ambient_runtime_shutdown() {
    let r = register_faux_provider(Default::default());
    r.set_responses(vec![
        FauxResponseStep::Message(message("ambient")),
        FauxResponseStep::Message(message("fallback")),
    ]);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .unwrap();
    let first = runtime
        .block_on(async { complete(r.get_model(None).unwrap(), context(), None).await })
        .unwrap();
    assert_eq!(first.read().unwrap().content, message("ambient").content);
    drop(runtime);
    let second = run(complete(r.get_model(None).unwrap(), context(), None)).unwrap();
    assert_eq!(second.read().unwrap().content, message("fallback").content);
    r.unregister();
}

#[test]
fn faux_pacing_needs_no_caller_timer_driver() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    runtime.block_on(async {
        let r = register_faux_provider(RegisterFauxProviderOptions {
            tokens_per_second: Some(1000.0),
            ..Default::default()
        });
        r.set_responses(vec![FauxResponseStep::Factory(Arc::new(|_, _, _, _| {
            Box::pin(async {
                let timer =
                    std::panic::catch_unwind(|| tokio::time::sleep(std::time::Duration::ZERO))
                        .map_err(|_| thrown("producer has no timer driver"))?;
                timer.await;
                Ok(message("paced"))
            })
        }))]);
        let response = stream(r.get_model(None).unwrap(), context(), None).unwrap();
        let mut iter = response.iter();
        let mut kinds = vec![];
        while let Some(event) = iter.next().await {
            kinds.push(event_name(&event));
        }
        assert_eq!(
            kinds,
            ["start", "text_start", "text_delta", "text_end", "done"]
        );
        let result = response.result().await;
        assert_eq!(result.read().unwrap().stop_reason, StopReason::Stop);
        assert_eq!(result.read().unwrap().content, message("paced").content);
        r.unregister();
    });
}
