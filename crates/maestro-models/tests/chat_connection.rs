mod support;
use maestro_models::*;
use support::{chat::*, conformance::*};
#[test]
fn connection_replaces_scripted_provider_without_caller_changes() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let events = collect(
        models.stream(m.clone(), context(), support::auth::local()),
        &m,
        73,
    );
    let mut updates = text(0, "hello");
    updates.push(done());
    let fake = ScriptedProvider::new(vec![steps(updates)]);
    let mut other = Models::new(std::sync::Arc::new(|| 73));
    other
        .register(m.clone(), std::sync::Arc::new(fake))
        .unwrap();
    assert_eq!(
        events,
        collect(
            other.stream(m.clone(), context(), support::auth::local()),
            &m,
            73
        )
    );
    assert_eq!(t.requests.lock().unwrap().len(), 1);
    let (models, m, t) = support::chat::fixture(success(), dialect());
    assert_eq!(
        support::block_on(models.complete(m, context(), support::auth::local())).stop_reason,
        Some(StopReason::Stop)
    );
    assert_eq!(t.requests.lock().unwrap().len(), 1);
}
#[test]
fn connection_serializes_projected_context_once() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let c = context();
    let original = c.clone();
    support::block_on(models.complete(m, c.clone(), support::auth::local()));
    assert_eq!(c, original);
    let p = payload(&t);
    assert_eq!(
        p["messages"][0],
        serde_json::json!({"role":"system","content":"synthetic secret"})
    );
    assert_eq!(p["messages"][1]["content"][0]["text"], "hello");
    assert_eq!(p["model"], "test");
    assert_eq!(p["stream"], true);
}
#[test]
fn connection_omits_empty_tools_unless_history_requires_them() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    support::block_on(models.complete(m, context(), support::auth::local()));
    assert!(payload(&t).get("tools").is_none());
    let mut d = dialect();
    d.strict_tools = true;
    d.tool_stream = true;
    let (models, m, t) = support::chat::fixture(success(), d);
    let mut c = context();
    c.tools.push(ToolDeclaration {
        name: "lookup".into(),
        description: "find".into(),
        parameters: serde_json::json!({"type":"object"}),
    });
    support::block_on(models.complete(m, c, support::auth::local()));
    assert_eq!(
        payload(&t)["tools"][0]["function"],
        serde_json::json!({"name":"lookup","description":"find","parameters":{"type":"object"},"strict":false})
    );
    assert_eq!(payload(&t)["tool_stream"], true);
}
#[test]
fn connection_uses_only_declared_dialect_fields() {
    for enabled in [false, true] {
        for field in [
            ChatOutputField::MaxTokens,
            ChatOutputField::MaxCompletionTokens,
        ] {
            let mut d = dialect();
            d.store = enabled;
            d.usage_in_stream = enabled;
            d.output_field = field;
            d.developer_role = enabled;
            let (models, mut m, t) = support::chat::fixture(success(), d);
            m.capabilities.reasoning = true;
            let _ = models;
            let mut models = Models::new(std::sync::Arc::new(|| 73));
            models
                .register(
                    m.clone(),
                    std::sync::Arc::new(ChatConnection::new(t.clone(), {
                        let mut d = dialect();
                        d.store = enabled;
                        d.usage_in_stream = enabled;
                        d.output_field = field;
                        d.developer_role = enabled;
                        d
                    })),
                )
                .unwrap();
            let mut o = support::auth::local();
            o.output_limit = Some(0);
            support::block_on(models.complete(m, context(), o));
            let p = payload(&t);
            assert_eq!(
                p.get("store"),
                enabled.then_some(&serde_json::Value::Bool(false))
            );
            assert_eq!(p.get("stream_options").is_some(), enabled);
            assert_eq!(
                p["messages"][0]["role"],
                if enabled { "developer" } else { "system" }
            );
            assert_eq!(
                p[if field == ChatOutputField::MaxTokens {
                    "max_tokens"
                } else {
                    "max_completion_tokens"
                }],
                0
            );
        }
    }
}
#[test]
fn connection_maps_all_declared_thinking_formats() {
    for format in [
        ChatThinkingFormat::Effort,
        ChatThinkingFormat::NestedEffort,
        ChatThinkingFormat::Toggle,
        ChatThinkingFormat::TemplateToggle,
        ChatThinkingFormat::TypedToggle,
    ] {
        let mut d = dialect();
        d.thinking_format = Some(format);
        let (_, mut m, t) = support::chat::fixture(success(), d.clone());
        m.capabilities.reasoning = true;
        let mut models = Models::new(std::sync::Arc::new(|| 73));
        models
            .register(
                m.clone(),
                std::sync::Arc::new(ChatConnection::new(t.clone(), d)),
            )
            .unwrap();
        let mut o = support::auth::local();
        o.thinking = ThinkingLevel::High;
        support::block_on(models.complete(m, context(), o));
        let p = payload(&t);
        match format {
            ChatThinkingFormat::Effort => assert_eq!(p["reasoning_effort"], "high"),
            ChatThinkingFormat::NestedEffort => assert_eq!(p["reasoning"]["effort"], "high"),
            ChatThinkingFormat::Toggle => assert_eq!(p["enable_thinking"], true),
            ChatThinkingFormat::TemplateToggle => assert_eq!(
                p["chat_template_kwargs"],
                serde_json::json!({"enable_thinking":true,"preserve_thinking":true})
            ),
            ChatThinkingFormat::TypedToggle => assert_eq!(p["thinking"]["type"], "enabled"),
        }
    }
}
#[test]
fn connection_gates_tool_choice_and_forwards_routing() {
    for supported in [false, true] {
        for choice in [
            ToolChoice::Auto,
            ToolChoice::None,
            ToolChoice::Required,
            ToolChoice::Function {
                name: "find".into(),
            },
        ] {
            let mut d = dialect();
            d.provider_routing = Some(
                serde_json::json!({"order":["x"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            );
            d.provider_options = d.provider_routing.clone();
            let (_, mut m, t) = support::chat::fixture(success(), d.clone());
            m.capabilities.tool_choice = supported;
            let mut models = Models::new(std::sync::Arc::new(|| 73));
            models
                .register(
                    m.clone(),
                    std::sync::Arc::new(ChatConnection::new(t.clone(), d)),
                )
                .unwrap();
            let mut o = support::auth::local();
            o.tool_choice = Some(choice.clone());
            support::block_on(models.complete(m, context(), o));
            let p = payload(&t);
            assert_eq!(p.get("tool_choice").is_some(), supported);
            assert_eq!(p["provider"], serde_json::json!({"order":["x"]}));
            assert_eq!(p["providerOptions"], p["provider"]);
        }
    }
}
#[test]
fn connection_cache_fields_and_affinity_are_explicit() {
    for pref in [None, Some("none"), Some("short"), Some("long")] {
        let mut d = dialect();
        d.prompt_cache_key = true;
        d.long_cache_retention = true;
        d.session_affinity_headers = true;
        let (_, mut m, t) = support::chat::fixture(success(), d.clone());
        m.capabilities.cache_preferences = ["none", "short", "long"].map(String::from).into();
        m.capabilities.session_affinity = true;
        let mut models = Models::new(std::sync::Arc::new(|| 73));
        models
            .register(
                m.clone(),
                std::sync::Arc::new(ChatConnection::new(t.clone(), d)),
            )
            .unwrap();
        let mut o = support::auth::local();
        o.cache_preference = pref.map(String::from);
        o.session_affinity = Some("session".into());
        support::block_on(models.complete(m, context(), o));
        let p = payload(&t);
        assert_eq!(
            p.get("prompt_cache_key").is_some(),
            matches!(pref, Some("short" | "long"))
        );
        assert_eq!(
            p.get("prompt_cache_retention").is_some(),
            pref == Some("long")
        );
        assert_eq!(
            t.requests.lock().unwrap()[0]
                .headers
                .contains_key("x-session-affinity"),
            matches!(pref, Some("short" | "long"))
        );
    }
}
#[test]
fn connection_auth_headers_and_no_exchange() {
    let t = std::sync::Arc::new(Transport::default());
    t.responses
        .lock()
        .unwrap()
        .push_back(Ok(response(200, vec![Ok(success())])));
    let connection = std::sync::Arc::new(ChatConnection::new(t.clone(), dialect()));
    assert!(connection.token_exchange().is_none());
    let mut models = Models::new(std::sync::Arc::new(|| 73));
    let m = support::chat::model();
    models.register(m.clone(), connection).unwrap();
    let mut o = support::auth::local();
    o.auth = Some(RequestAuth::Secret {
        secret: SecretString::new("literal".into()),
        source: None,
    });
    support::block_on(models.complete(m, context(), o));
    assert_eq!(
        t.requests.lock().unwrap()[0].headers["authorization"],
        "Bearer literal"
    );
}
fn replay(content: Vec<AssistantContent>) -> AssistantMessage {
    AssistantMessage {
        provider: "local".into(),
        protocol: "chat-completions".into(),
        model: "test".into(),
        timestamp: 1,
        content,
        usage: Usage::default(),
        stop_reason: Some(StopReason::Stop),
        failure: None,
        response_model: None,
        response_id: None,
    }
}
#[test]
fn connection_replays_thinking_as_text_or_signed_field() {
    for as_text in [false, true] {
        let mut d = dialect();
        d.thinking_as_text = as_text;
        let (models, m, t) = support::chat::fixture(success(), d);
        let mut c = context();
        c.messages = vec![Message::Assistant(replay(vec![
            AssistantContent::Thinking(ThinkingContent::Readable {
                text: "idea".into(),
                signature: Some("reasoning".into()),
            }),
            AssistantContent::Text(TextContent {
                text: "answer".into(),
                replay_metadata: None,
            }),
        ]))];
        support::block_on(models.complete(m, c, support::auth::local()));
        let p = payload(&t);
        if as_text {
            assert_eq!(
                p["messages"][1]["content"],
                serde_json::json!([{"type":"text","text":"idea"},{"type":"text","text":"answer"}])
            );
        } else {
            assert_eq!(p["messages"][1]["content"], "answer");
            assert_eq!(p["messages"][1]["reasoning"], "idea");
        }
    }
}
#[test]
fn connection_replays_multiple_thinking_blocks_with_two_linefeeds() {
    let mut d = dialect();
    d.thinking_as_text = true;
    let (models, m, t) = support::chat::fixture(success(), d);
    let mut c = context();
    c.messages = vec![Message::Assistant(replay(vec![
        AssistantContent::Thinking(ThinkingContent::Readable {
            text: "one".into(),
            signature: None,
        }),
        AssistantContent::Thinking(ThinkingContent::Readable {
            text: "two".into(),
            signature: None,
        }),
    ]))];
    support::block_on(models.complete(m, c, support::auth::local()));
    assert_eq!(
        payload(&t)["messages"][1]["content"],
        serde_json::json!([{"type":"text","text":"one\n\ntwo"}])
    );
}

#[test]
fn connection_replay_uses_ecmascript_whitespace() {
    for s in ["\u{feff}", "\u{a0}", "\u{2028}", "\u{2029}", "\u{85}"] {
        let (models, m, t) = support::chat::fixture(success(), dialect());
        let mut c = context();
        c.messages = vec![Message::Assistant(replay(vec![AssistantContent::Text(
            TextContent {
                text: s.into(),
                replay_metadata: None,
            },
        )]))];
        support::block_on(models.complete(m, c, support::auth::local()));
        let p = payload(&t);
        assert_eq!(
            p["messages"].as_array().unwrap().len(),
            if s == "\u{85}" { 2 } else { 1 }
        );
        if s == "\u{85}" {
            assert_eq!(p["messages"][1]["content"], s);
        }
    }
}
#[test]
fn connection_batches_tool_images_after_results() {
    let mut d = dialect();
    d.assistant_after_tool_result = true;
    d.tool_result_name = true;
    let (models, m, t) = support::chat::fixture(success(), d);
    let mut c = context();
    let a = replay(vec![AssistantContent::ToolCall(ToolCall::new(
        "id".into(),
        "look".into(),
        serde_json::Map::new(),
        None,
    ))]);
    c.messages = vec![
        Message::Assistant(a),
        Message::ToolResult(ToolResultMessage {
            tool_call_id: "id".into(),
            tool_name: "look".into(),
            content: vec![InputContent::Text(TextContent {
                text: "result".into(),
                replay_metadata: None,
            })],
            details: Some(serde_json::json!({"private":true})),
            is_error: false,
            timestamp: 2,
        }),
    ];
    c.messages.push(context().messages[0].clone());
    support::block_on(models.complete(m, c, support::auth::local()));
    let p = payload(&t);
    assert_eq!(
        p["messages"][2],
        serde_json::json!({"role":"tool","tool_call_id":"id","name":"look","content":"result"})
    );
    assert_eq!(
        p["messages"][3]["content"],
        "I have processed the tool results."
    );
}
#[test]
fn connection_places_ephemeral_cache_markers() {
    let mut d = dialect();
    d.cache_control = Some(ChatCacheControl::Ephemeral);
    let (_, mut m, t) = support::chat::fixture(success(), d.clone());
    m.capabilities.cache_preferences.insert("short".into());
    let mut models = Models::new(std::sync::Arc::new(|| 73));
    models
        .register(
            m.clone(),
            std::sync::Arc::new(ChatConnection::new(t.clone(), d)),
        )
        .unwrap();
    let mut o = support::auth::local();
    o.cache_preference = Some("short".into());
    support::block_on(models.complete(m, context(), o));
    let p = payload(&t);
    assert_eq!(
        p["messages"][0]["content"][0]["cache_control"],
        serde_json::json!({"type":"ephemeral"})
    );
    assert_eq!(
        p["messages"][1]["content"][0]["cache_control"],
        serde_json::json!({"type":"ephemeral"})
    );
}
#[test]
fn connection_compact_json_has_stable_keys_and_ecmascript_numbers() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let mut c = context();
    let arguments = serde_json::json!({"10":-0.0,"2":1e-7,"01":1e-6,"a":1e20,"z":1e21})
        .as_object()
        .unwrap()
        .clone();
    c.messages = vec![Message::Assistant(replay(vec![
        AssistantContent::ToolCall(ToolCall::new("id".into(), "f".into(), arguments, None)),
    ]))];
    support::block_on(models.complete(m, c, support::auth::local()));
    assert_eq!(
        payload(&t)["messages"][1]["tool_calls"][0]["function"]["arguments"],
        "{\"2\":1e-7,\"10\":0,\"01\":0.000001,\"a\":100000000000000000000,\"z\":1e+21}"
    );
}
#[test]
fn connection_normalizes_foreign_ids_without_collisions() {
    let p = ChatConnection::new(std::sync::Arc::new(Transport::default()), dialect());
    let m = support::chat::model();
    let mut a = replay(vec![
        AssistantContent::ToolCall(ToolCall::new(
            "abc!|x".into(),
            "f".into(),
            serde_json::Map::new(),
            None,
        )),
        AssistantContent::ToolCall(ToolCall::new(
            "abc?|y".into(),
            "f".into(),
            serde_json::Map::new(),
            None,
        )),
    ]);
    a.protocol = "foreign".into();
    assert_eq!(p.normalize_tool_call_id("abc!|x", &m, &a), "abc_");
    assert_eq!(p.normalize_tool_call_id("abc?|y", &m, &a), "abc__1");
    assert_eq!(p.normalize_tool_call_id("😀|x", &m, &a), "__");
}
#[test]
fn connection_endpoint_append_is_url_not_path_resolution() {
    for endpoint in [
        "http://localhost/v1",
        "http://localhost/v1/",
        "http://localhost/a/../v1",
    ] {
        let (models, mut m, t) = support::chat::fixture(success(), dialect());
        let _ = models;
        m.endpoint = endpoint.into();
        let mut models = Models::new(std::sync::Arc::new(|| 73));
        models
            .register(
                m.clone(),
                std::sync::Arc::new(ChatConnection::new(t.clone(), dialect())),
            )
            .unwrap();
        support::block_on(models.complete(m, context(), support::auth::local()));
        assert_eq!(
            t.requests.lock().unwrap()[0].url,
            "http://localhost/v1/chat/completions"
        );
    }
}
#[test]
fn connection_requires_finish_reason_with_optional_marker() {
    for (reason, outcome) in [
        ("stop", Ok(StopReason::Stop)),
        ("end", Ok(StopReason::Stop)),
        ("length", Ok(StopReason::Length)),
        ("tool_calls", Ok(StopReason::ToolUse)),
        ("function_call", Ok(StopReason::ToolUse)),
        ("unknown", Err(Failure::MalformedStream)),
        ("content_filter", Err(Failure::AdapterFailed)),
        ("network_error", Err(Failure::Transport)),
        (
            "model_context_window_exceeded",
            Err(Failure::ContextOverflow),
        ),
    ] {
        for marker in [false, true] {
            let mut bytes = frames(vec![
                serde_json::json!({"choices":[{"delta":{},"finish_reason":reason}]}),
            ]);
            if marker {
                bytes.extend(b"data: [DONE]\n\n");
            }
            let (events, _) = support::chat::run(bytes);
            let m = terminal(&events);
            match outcome {
                Ok(r) => assert_eq!(m.stop_reason, Some(r)),
                Err(f) => assert_eq!(m.failure, Some(f)),
            }
        }
    }
    for bytes in [
        vec![],
        b"data: [DONE]\n\n".to_vec(),
        frames(vec![serde_json::json!({"choices":[{"delta":{}}]})]),
    ] {
        assert_eq!(
            terminal(&support::chat::run(bytes).0).failure,
            Some(Failure::IncompleteStream)
        );
    }
}
#[test]
fn connection_decodes_fragmented_sse_and_utf8() {
    for ending in ["\n", "\r", "\r\n"] {
        let bytes=format!(": comment{ending}event: message{ending}unknown:x{ending}data: {{\"choices\":[{{\"delta\":{{\"content\":\"😀你好\"}},\"finish_reason\":\"stop\"}}]}}{ending}{ending}").into_bytes();
        for split in 0..=bytes.len() {
            let (models, m, t) = support::chat::fixture(vec![], dialect());
            t.responses.lock().unwrap().clear();
            t.responses.lock().unwrap().push_back(Ok(response(
                200,
                vec![Ok(bytes[..split].to_vec()), Ok(bytes[split..].to_vec())],
            )));
            let out = support::block_on(models.complete(m, context(), support::auth::local()));
            assert_eq!(out.failure, None, "{ending:?}/{split}");
            assert_eq!(
                out.content,
                vec![AssistantContent::Text(TextContent {
                    text: "😀你好".into(),
                    replay_metadata: None
                })]
            );
        }
    }
}
#[test]
fn connection_failure_after_finish_is_not_success_or_retry() {
    for tail in [
        b"data: {bad}\n\n".to_vec(),
        b"data: \xff\n\n".to_vec(),
        b"data: {".to_vec(),
    ] {
        let mut bytes = success();
        bytes.extend(tail);
        let (events, t) = support::chat::run(bytes);
        assert_eq!(terminal(&events).failure, Some(Failure::MalformedStream));
        assert_eq!(terminal(&events).content.len(), 1);
        assert_eq!(t.requests.lock().unwrap().len(), 1);
    }
}
#[test]
fn connection_identity_is_sticky_and_requested_identity_is_unchanged() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"id":"one","model":"routed","choices":[]}),
        serde_json::json!({"id":"two","model":"later","choices":[{"delta":{},"finish_reason":"stop"}]}),
    ]));
    let m = terminal(&events);
    assert_eq!(m.response_id.as_deref(), Some("one"));
    assert_eq!(m.response_model.as_deref(), Some("routed"));
    assert_eq!(m.model, "test");
}
#[test]
fn connection_normalizes_literal_raw_usage_snapshots() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"choices":[{"delta":{},"finish_reason":"stop"}]}),
        serde_json::json!({"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":5,"prompt_tokens_details":{"cached_tokens":50,"cache_write_tokens":30}}}),
    ]));
    let u = &terminal(&events).usage;
    assert_eq!(
        (
            u.input,
            u.output,
            u.cache_read,
            u.cache_write,
            u.total_tokens,
            u.reported
        ),
        (50, 5, 20, 30, 105, true)
    );
}
#[test]
fn connection_invalid_usage_preserves_prior_valid_report() {
    for invalid in [
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!(1e30),
    ] {
        let (events, _) = support::chat::run(frames(vec![
            serde_json::json!({"usage":{"prompt_tokens":7},"choices":[]}),
            serde_json::json!({"usage":{"prompt_tokens":invalid},"choices":[]}),
        ]));
        assert_eq!(terminal(&events).failure, Some(Failure::MalformedStream));
        assert_eq!(terminal(&events).usage.input, 7);
    }
}
#[test]
fn connection_reasoning_field_precedence_and_replay() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"choices":[{"delta":{"reasoning_content":"first","reasoning":"ignored","reasoning_text":"ignored"}}]}),
        serde_json::json!({"choices":[{"delta":{"reasoning":"second"},"finish_reason":"stop"}]}),
    ]));
    assert_eq!(
        terminal(&events).content,
        vec![AssistantContent::Thinking(ThinkingContent::Readable {
            text: "firstsecond".into(),
            signature: Some("reasoning_content".into())
        })]
    );
}
#[test]
fn connection_interleaves_blocks_and_coalesces_tool_indices() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"choices":[{"delta":{"content":"text","reasoning":"idea","tool_calls":[{"index":0,"id":"a","function":{"name":"one","arguments":"{"}},{"index":1,"id":"b","function":{"name":"two","arguments":"{}"}}]}}]}),
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"alias","function":{"name":"changed","arguments":"}"}}]},"finish_reason":"tool_calls"}]}),
    ]));
    let m = terminal(&events);
    assert_eq!(m.failure, None);
    assert_eq!(m.content.len(), 4);
    let AssistantContent::ToolCall(call) = &m.content[2] else {
        panic!()
    };
    assert_eq!(
        (&call.id, &call.name),
        (&"a".to_string(), &"one".to_string())
    );
    assert_eq!(call.arguments(), Some(&serde_json::Map::new()));
}
#[test]
fn connection_tool_json_is_private_until_strict_end() {
    for json in ["{}", "[]", "1", "{", "{bad}"] {
        let (events, _) = support::chat::run(frames(vec![
            serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"id","function":{"name":"f","arguments":json}}]},"finish_reason":"tool_calls"}]}),
        ]));
        let final_record = terminal(&events);
        assert_eq!(
            final_record.failure,
            if json == "{}" {
                None
            } else {
                Some(Failure::MalformedStream)
            }
        );
        for event in &events {
            if let ModelEvent::ToolCallDelta { partial, .. } = event {
                let AssistantContent::ToolCall(t) = &partial.content[0] else {
                    panic!()
                };
                assert!(t.arguments().is_none());
            }
        }
    }
}
#[test]
fn connection_late_tool_metadata_updates_only_later_snapshots() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{"}}]}}]}),
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"id","function":{"name":"f","arguments":"}"}}],"reasoning_details":[{"type":"reasoning.encrypted","id":"id","data":"cipher"}]},"finish_reason":"tool_calls"}]}),
    ]));
    let AssistantContent::ToolCall(first) = &snapshot(&events[1]).content[0] else {
        panic!()
    };
    assert_eq!(first.id, "");
    let AssistantContent::ToolCall(last) = &terminal(&events).content[0] else {
        panic!()
    };
    assert_eq!(last.id, "id");
    assert!(
        last.replay_metadata
            .as_ref()
            .is_some_and(|s| s.contains("cipher"))
    );
}
#[test]
fn connection_status_and_error_envelopes_keep_safe_categories() {
    for (error, f) in [
        (
            serde_json::json!({"code":"invalid_api_key"}),
            Failure::AuthenticationFailed,
        ),
        (
            serde_json::json!({"code":"insufficient_quota"}),
            Failure::QuotaExceeded,
        ),
        (
            serde_json::json!({"code":"rate_limit_exceeded"}),
            Failure::Throttled,
        ),
        (
            serde_json::json!({"type":"overloaded_error"}),
            Failure::Overloaded,
        ),
        (
            serde_json::json!({"message":"prompt is too long"}),
            Failure::ContextOverflow,
        ),
        (
            serde_json::json!({"message":"sentinel"}),
            Failure::AdapterFailed,
        ),
    ] {
        let (events, _) = support::chat::run(frames(vec![serde_json::json!({"error":error})]));
        assert_eq!(terminal(&events).failure, Some(f));
    }
}
#[test]
fn connection_overflow_patterns_do_not_swallow_rate_limits() {
    for text in [
        "input token count 99 exceeds the maximum",
        "maximum prompt length is 12",
        "maximum context length is 12 tokens",
        "exceeds the limit of 12",
        "too large for model with 12 maximum context length",
        "413\u{feff}status code (no body)",
    ] {
        let (events, _) =
            support::chat::run(frames(vec![serde_json::json!({"error":{"message":text}})]));
        assert_eq!(
            terminal(&events).failure,
            Some(Failure::ContextOverflow),
            "{text}"
        );
    }
    for text in [
        "rate limit: too many tokens",
        "too many requests: token limit exceeded",
        "Throttling error: maximum prompt length is 12",
    ] {
        assert_eq!(
            terminal(
                &support::chat::run(frames(vec![serde_json::json!({"error":{"message":text}})])).0
            )
            .failure,
            Some(Failure::Throttled)
        );
    }
    for text in [
        "maximum prompt length is １２",
        "input token count\nexceeds the maximum",
        "413\u{85}(no body)",
    ] {
        assert_eq!(
            terminal(
                &support::chat::run(frames(vec![serde_json::json!({"error":{"message":text}})])).0
            )
            .failure,
            Some(Failure::AdapterFailed)
        );
    }
}
#[test]
fn connection_secrets_never_enter_observables() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"error":{"message":"sentinel-private-secret"}}),
    ]));
    assert!(!format!("{events:?}").contains("sentinel-private-secret"));
    assert!(
        !terminal(&events)
            .failure
            .unwrap()
            .to_string()
            .contains("sentinel-private-secret")
    );
}
#[test]
fn connection_setup_retry_eligibility_and_overrides() {
    for retries in [None, Some(0)] {
        let (models, m, t) = support::chat::fixture(vec![], dialect());
        t.responses.lock().unwrap().clear();
        for _ in 0..3 {
            t.responses
                .lock()
                .unwrap()
                .push_back(Ok(response(429, vec![])));
        }
        let mut o = support::auth::local();
        o.max_retries = retries;
        let out = support::block_on(models.complete(m, context(), o));
        assert_eq!(out.failure, Some(Failure::Throttled));
        assert_eq!(
            t.requests.lock().unwrap().len(),
            if retries == Some(0) { 1 } else { 3 }
        );
    }
}
#[test]
fn connection_retry_delays_precedence_and_invalid_values() {
    for (ms, seconds, expected) in [
        ("0", "9", 0.0),
        ("1.25", "9", 1.25),
        ("-1", "2", 2000.0),
        ("1e300", "3", 3000.0),
        ("bad", "bad", 500.0),
    ] {
        let (models, m, t) = support::chat::fixture(success(), dialect());
        let mut r = response(429, vec![]);
        r.headers.insert("retry-after-ms".into(), ms.into());
        r.headers.insert("retry-after".into(), seconds.into());
        t.responses.lock().unwrap().push_front(Ok(r));
        support::block_on(models.complete(m, context(), support::auth::local()));
        assert_eq!(
            t.delays.lock().unwrap()[0],
            std::time::Duration::from_secs_f64(expected / 1000.0)
        );
    }
}
#[test]
fn connection_retry_http_dates_use_supplied_utc_clock() {
    for date in [
        "Sun, 06 Nov 1994 08:49:37 GMT",
        "Sunday, 06-Nov-94 08:49:37 GMT",
        "Sun Nov  6 08:49:37 1994",
    ] {
        let (models, m, t) = support::chat::fixture(success(), dialect());
        let mut r = response(429, vec![]);
        r.headers.insert("retry-after".into(), date.into());
        t.responses.lock().unwrap().push_front(Ok(r));
        support::block_on(models.complete(m, context(), support::auth::local()));
        assert_eq!(
            t.delays.lock().unwrap()[0],
            std::time::Duration::from_millis(784111777000)
        );
    }
}
#[test]
fn connection_fallback_backoff_has_exact_cap_and_jitter() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    for _ in 0..6 {
        t.responses
            .lock()
            .unwrap()
            .push_front(Err(Failure::Transport));
    }
    let mut o = support::auth::local();
    o.max_retries = Some(6);
    support::block_on(models.complete(m, context(), o));
    assert_eq!(
        *t.delays.lock().unwrap(),
        [500, 1000, 2000, 4000, 8000, 8000].map(std::time::Duration::from_millis)
    );
}
#[test]
fn connection_cancellation_drops_setup_read_and_backoff() {
    for phase in 0..3 {
        let gate = Gate::default();
        let t = std::sync::Arc::new(Transport {
            send_gate: (phase == 0).then(|| gate.clone()),
            wait_gate: (phase == 2).then(|| gate.clone()),
            ..Transport::default()
        });
        if phase == 1 {
            t.responses.lock().unwrap().push_back(Ok(ChatHttpResponse {
                status: 200,
                headers: Default::default(),
                body: Box::new(GateBody(gate.clone(), vec![Ok(success())].into())),
            }));
        } else if phase == 2 {
            t.responses
                .lock()
                .unwrap()
                .push_back(Err(Failure::Transport));
        }
        let mut models = Models::new(std::sync::Arc::new(|| 73));
        let m = support::chat::model();
        models
            .register(
                m.clone(),
                std::sync::Arc::new(ChatConnection::new(t.clone(), dialect())),
            )
            .unwrap();
        let o = support::auth::local();
        let cancel = o.cancellation.clone();
        let mut stream = models.stream(m, context(), o);
        let counter = std::sync::Arc::new(WakeCounter::default());
        {
            let mut next = std::pin::pin!(stream.next());
            assert!(poll(next.as_mut(), &counter).is_pending());
            cancel.cancel();
            let std::task::Poll::Ready(Some(ModelEvent::Error { reason, .. })) =
                poll(next.as_mut(), &counter)
            else {
                panic!()
            };
            assert_eq!(reason, StopReason::Aborted);
        }
        assert_eq!(support::block_on(stream.next()), None);
        assert!(gate.1.load(std::sync::atomic::Ordering::SeqCst) > 0);
    }
}
#[test]
fn connection_header_timeout_is_per_attempt_and_zero_is_immediate() {
    for timeout in [None, Some(123), Some(0)] {
        let (models, m, t) = support::chat::fixture(success(), dialect());
        let mut o = support::auth::local();
        o.timeout_ms = timeout;
        o.max_retries = Some(0);
        let out = support::block_on(models.complete(m, context(), o));
        if timeout == Some(0) {
            assert_eq!(out.failure, Some(Failure::SetupTimeout));
            assert!(t.requests.lock().unwrap().is_empty());
        } else {
            assert_eq!(out.failure, None);
            assert_eq!(*t.timeouts.lock().unwrap(), vec![timeout.unwrap_or(600000)]);
        }
    }
}
#[test]
fn connection_has_no_body_idle_or_whole_operation_timeout() {
    let gate = Gate::default();
    let (models, m, t) = support::chat::fixture(success(), dialect());
    t.responses.lock().unwrap().clear();
    t.responses.lock().unwrap().push_back(Ok(ChatHttpResponse {
        status: 200,
        headers: Default::default(),
        body: Box::new(GateBody(gate.clone(), vec![Ok(success())].into())),
    }));
    let mut stream = models.stream(m, context(), support::auth::local());
    let counter = std::sync::Arc::new(WakeCounter::default());
    {
        let mut next = std::pin::pin!(stream.next());
        assert!(poll(next.as_mut(), &counter).is_pending());
    }
    drop(stream);
    assert_eq!(gate.1.load(std::sync::atomic::Ordering::SeqCst), 1);
}
#[test]
fn connection_construction_auth_and_body_failures_never_retry() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let out = support::block_on(models.complete(m, context(), StreamOptions::default()));
    assert_eq!(out.failure, Some(Failure::MissingAuthentication));
    assert!(t.requests.lock().unwrap().is_empty());
    let (models, m, t) = support::chat::fixture(success(), dialect());
    t.responses.lock().unwrap().clear();
    t.responses
        .lock()
        .unwrap()
        .push_back(Ok(response(200, vec![Err(Failure::Transport)])));
    let out = support::block_on(models.complete(m, context(), support::auth::local()));
    assert_eq!(out.failure, Some(Failure::Transport));
    assert_eq!(t.requests.lock().unwrap().len(), 1);
}
#[test]
fn connection_accepts_integral_decimal_usage_counters() {
    let(events,_)=support::chat::run(b"data: {\"usage\":{\"prompt_tokens\":7.0,\"completion_tokens\":-0.0},\"choices\":[{\"finish_reason\":\"stop\"}]}\n\n".to_vec());
    assert_eq!(terminal(&events).failure, None);
    assert_eq!(terminal(&events).usage.input, 7);
}
#[test]
fn connection_setup_status_matrix_and_retry_header_overrides() {
    for (status, header, retry) in [
        (408, None, true),
        (409, None, true),
        (429, None, true),
        (500, None, true),
        (599, None, true),
        (400, None, false),
        (401, None, false),
        (403, None, false),
        (404, None, false),
        (302, None, false),
        (401, Some("true"), true),
        (500, Some("false"), false),
        (400, Some("True"), false),
    ] {
        let (models, m, t) = support::chat::fixture(success(), dialect());
        let mut r = response(status, vec![Ok(b"{}".to_vec())]);
        if let Some(h) = header {
            r.headers.insert("x-should-retry".into(), h.into());
        }
        t.responses.lock().unwrap().push_front(Ok(r));
        let mut o = support::auth::local();
        o.max_retries = Some(1);
        let out = support::block_on(models.complete(m, context(), o));
        assert_eq!(t.requests.lock().unwrap().len(), if retry { 2 } else { 1 });
        assert_eq!(out.failure.is_none(), retry);
    }
}
#[test]
fn connection_tool_results_do_not_bridge_to_existing_assistant() {
    let mut d = dialect();
    d.assistant_after_tool_result = true;
    let (models, m, t) = support::chat::fixture(success(), d);
    let mut c = context();
    c.messages = vec![
        Message::Assistant(replay(vec![AssistantContent::ToolCall(ToolCall::new(
            "id".into(),
            "f".into(),
            serde_json::Map::new(),
            None,
        ))])),
        Message::ToolResult(ToolResultMessage {
            tool_call_id: "id".into(),
            tool_name: "f".into(),
            content: vec![],
            details: None,
            is_error: false,
            timestamp: 1,
        }),
        Message::Assistant(replay(vec![AssistantContent::Text(TextContent {
            text: "answer".into(),
            replay_metadata: None,
        })])),
    ];
    support::block_on(models.complete(m, c, support::auth::local()));
    assert_eq!(payload(&t)["messages"].as_array().unwrap().len(), 4);
}
#[test]
fn connection_affinity_uses_three_declared_wire_names() {
    let mut d = dialect();
    d.session_affinity_headers = true;
    let (_, mut m, t) = support::chat::fixture(success(), d.clone());
    m.capabilities.cache_preferences.insert("short".into());
    m.capabilities.session_affinity = true;
    let mut models = Models::new(std::sync::Arc::new(|| 73));
    models
        .register(
            m.clone(),
            std::sync::Arc::new(ChatConnection::new(t.clone(), d)),
        )
        .unwrap();
    let mut o = support::auth::local();
    o.cache_preference = Some("short".into());
    o.session_affinity = Some("session".into());
    support::block_on(models.complete(m, context(), o));
    let request = t.requests.lock().unwrap();
    for name in ["session_id", "x-client-request-id", "x-session-affinity"] {
        assert_eq!(
            request[0].headers.get(name).map(String::as_str),
            Some("session")
        );
    }
    assert!(!request[0].headers.contains_key("x-session-id"));
}
#[test]
fn connection_opaque_replay_uses_compact_binary64_spelling() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let mut c = context();
    c.messages = vec![Message::Assistant(replay(vec![
        AssistantContent::ToolCall(ToolCall::new(
            "id".into(),
            "f".into(),
            serde_json::Map::new(),
            Some("{\"n\":9007199254740993,\"zero\":-0.0}".into()),
        )),
    ]))];
    support::block_on(models.complete(m, c, support::auth::local()));
    let request = t.requests.lock().unwrap();
    let body = std::str::from_utf8(&request[0].body).unwrap();
    assert!(body.contains("\"n\":9007199254740992"));
    assert!(body.contains("\"zero\":0"));
}
#[test]
fn connection_error_status_classification_is_secret_safe() {
    for (status, error, expected) in [
        (401, serde_json::json!({}), Failure::AuthenticationFailed),
        (403, serde_json::json!({}), Failure::AuthenticationFailed),
        (
            429,
            serde_json::json!({"code":"insufficient_quota","message":"sentinel"}),
            Failure::QuotaExceeded,
        ),
        (503, serde_json::json!({}), Failure::Overloaded),
        (
            418,
            serde_json::json!({"message":"sentinel"}),
            Failure::HttpStatus { status: 418 },
        ),
        (
            400,
            serde_json::json!({"message":"prompt is too long"}),
            Failure::ContextOverflow,
        ),
    ] {
        let (models, m, t) = support::chat::fixture(success(), dialect());
        t.responses.lock().unwrap().clear();
        t.responses.lock().unwrap().push_back(Ok(response(
            status,
            vec![Ok(
                serde_json::to_vec(&serde_json::json!({"error":error})).unwrap()
            )],
        )));
        let mut o = support::auth::local();
        o.max_retries = Some(0);
        let out = support::block_on(models.complete(m, context(), o));
        assert_eq!(out.failure, Some(expected));
        assert!(!format!("{out:?}").contains("sentinel"));
    }
}
#[test]
fn connection_http_date_fifty_year_rule_uses_full_date() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let mut r = response(429, vec![]);
    r.headers.insert(
        "retry-after".into(),
        "Friday, 02-Jan-20 00:00:00 GMT".into(),
    );
    t.responses.lock().unwrap().push_front(Ok(r));
    support::block_on(models.complete(m, context(), support::auth::local()));
    assert_eq!(t.delays.lock().unwrap()[0], std::time::Duration::ZERO);
}
#[test]
fn connection_http_date_fifty_year_boundary_allows_exactly_fifty_years() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let mut r = response(429, vec![]);
    r.headers.insert(
        "retry-after".into(),
        "Wednesday, 01-Jan-20 00:00:00 GMT".into(),
    );
    t.responses.lock().unwrap().push_front(Ok(r));
    support::block_on(models.complete(m, context(), support::auth::local()));
    assert_eq!(
        t.delays.lock().unwrap()[0],
        std::time::Duration::from_millis(1577836800000)
    );
}
#[test]
fn connection_and_fake_share_late_tool_metadata_conformance() {
    let bytes = frames(vec![
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"{"}}]}}]}),
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":0,"id":"id","function":{"name":"f","arguments":"}"}}]},"finish_reason":"tool_calls"}]}),
    ]);
    let (models, m, _) = support::chat::fixture(bytes, dialect());
    let actual = collect(
        models.stream(m.clone(), context(), support::auth::local()),
        &m,
        73,
    );
    let fake = std::sync::Arc::new(ScriptedProvider::new(vec![steps(vec![
        ProviderUpdate::ToolCallStart {
            content_index: 0,
            id: "".into(),
            name: "".into(),
            replay_metadata: None,
        },
        ProviderUpdate::ToolCallDelta {
            content_index: 0,
            delta: "{".into(),
        },
        ProviderUpdate::ToolCallMetadata {
            content_index: 0,
            id: Some("id".into()),
            name: Some("f".into()),
            replay_metadata: None,
        },
        ProviderUpdate::ToolCallDelta {
            content_index: 0,
            delta: "}".into(),
        },
        ProviderUpdate::ToolCallEnd { content_index: 0 },
        ProviderUpdate::Done {
            reason: StopReason::ToolUse,
        },
    ])]));
    let mut models = Models::new(std::sync::Arc::new(|| 73));
    models.register(m.clone(), fake).unwrap();
    assert_eq!(
        actual,
        collect(
            models.stream(m.clone(), context(), support::auth::local()),
            &m,
            73
        )
    );
}
#[test]
fn connection_raw_usage_precedence_reasoning_and_snapshot_replacement() {
    for (raw, expected) in [
        (
            serde_json::json!({"prompt_tokens":100,"completion_tokens":33,"completion_tokens_details":{"reasoning_tokens":21},"prompt_cache_hit_tokens":40}),
            (60, 33, 40, 0, 133),
        ),
        (
            serde_json::json!({"prompt_tokens":3,"completion_tokens":1,"prompt_cache_hit_tokens":40,"prompt_tokens_details":{"cached_tokens":0}}),
            (3, 1, 0, 0, 4),
        ),
        (
            serde_json::json!({"prompt_tokens":3,"prompt_tokens_details":{"cached_tokens":10,"cache_write_tokens":30}}),
            (0, 0, 0, 30, 30),
        ),
        (serde_json::json!({}), (0, 0, 0, 0, 0)),
    ] {
        let (events, _) = support::chat::run(frames(vec![
            serde_json::json!({"usage":{"prompt_tokens":99},"choices":[]}),
            serde_json::json!({"choices":[{"usage":raw,"delta":{},"finish_reason":"stop"}]}),
        ]));
        let u = &terminal(&events).usage;
        assert_eq!(
            (
                u.input,
                u.output,
                u.cache_read,
                u.cache_write,
                u.total_tokens
            ),
            expected
        );
        assert!(u.reported);
    }
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"usage":{"prompt_tokens":7},"choices":[{"usage":{"prompt_tokens":99},"finish_reason":"stop"}]}),
    ]));
    assert_eq!(terminal(&events).usage.input, 7);
}
#[test]
fn connection_sse_multiline_fields_and_exact_marker_boundary() {
    let bytes=b": ignored\ndata:{\ndata: \"choices\":[{\"delta\":{\"content\":\"x\"},\"finish_reason\":\"stop\"}]\ndata: }\n\ndata: [DONE]\n\ndata: {bad}\n\n".to_vec();
    let (events, _) = support::chat::run(bytes);
    assert_eq!(terminal(&events).failure, None);
    for data in ["[DONE]junk", "\"\\ud800\"", "not json"] {
        let mut bytes = success();
        bytes.extend(format!("data: {data}\n\n").as_bytes());
        assert_eq!(
            terminal(&support::chat::run(bytes).0).failure,
            Some(Failure::MalformedStream)
        );
    }
}
#[test]
fn connection_custom_auth_case_overlays_and_invalid_generated_headers() {
    for supplied in [false, true] {
        let mut d = dialect();
        d.auth_header = "X-Api-Key".into();
        d.auth_prefix = "Token ".into();
        let (models, m, t) = support::chat::fixture(success(), d);
        let mut o = support::auth::local();
        o.auth = Some(RequestAuth::Secret {
            secret: SecretString::new("secret".into()),
            source: None,
        });
        if supplied {
            o.headers.insert("x-API-key".into(), "override".into());
        }
        support::block_on(models.complete(m, context(), o));
        assert_eq!(
            t.requests.lock().unwrap()[0].headers["x-api-key"],
            if supplied { "override" } else { "Token secret" }
        );
    }
    let mut d = dialect();
    d.auth_header = "invalid name".into();
    let (models, m, t) = support::chat::fixture(success(), d);
    let mut o = support::auth::local();
    o.auth = Some(RequestAuth::Secret {
        secret: SecretString::new("private".into()),
        source: None,
    });
    let out = support::block_on(models.complete(m, context(), o));
    assert_eq!(out.failure, Some(Failure::InvalidRequestHeaders));
    assert!(t.requests.lock().unwrap().is_empty());
}
#[test]
fn connection_supported_tool_images_form_one_following_user_batch() {
    let mut d = dialect();
    d.assistant_after_tool_result = true;
    let (_, mut m, t) = support::chat::fixture(success(), d.clone());
    m.input.push("image".into());
    let mut models = Models::new(std::sync::Arc::new(|| 73));
    models
        .register(
            m.clone(),
            std::sync::Arc::new(ChatConnection::new(t.clone(), d)),
        )
        .unwrap();
    let mut c = context();
    let calls = ["a", "b"]
        .map(|id| {
            AssistantContent::ToolCall(ToolCall::new(
                id.into(),
                "f".into(),
                serde_json::Map::new(),
                None,
            ))
        })
        .to_vec();
    c.messages = vec![Message::Assistant(replay(calls))];
    for id in ["a", "b"] {
        c.messages.push(Message::ToolResult(ToolResultMessage {
            tool_call_id: id.into(),
            tool_name: "f".into(),
            content: vec![InputContent::Image(ImageContent {
                data: format!("😀{id}"),
                mime_type: "image/png".into(),
            })],
            details: None,
            is_error: false,
            timestamp: 1,
        }));
    }
    support::block_on(models.complete(m, c, support::auth::local()));
    let p = payload(&t);
    assert_eq!(p["messages"][2]["role"], "tool");
    assert_eq!(p["messages"][3]["role"], "tool");
    assert_eq!(p["messages"][2]["content"], "(see attached image)");
    assert_eq!(p["messages"][4]["role"], "assistant");
    let parts = p["messages"][5]["content"].as_array().unwrap();
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0]["text"], "Attached image(s) from tool result:");
    assert_eq!(parts[1]["image_url"]["url"], "data:image/png;base64,😀a");
    assert_eq!(parts[2]["image_url"]["url"], "data:image/png;base64,😀b");
}
#[test]
fn connection_error_envelope_identity_is_not_response_data() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"id":"error-secret","model":"error-secret","error":{"message":"error-secret"}}),
    ]));
    assert!(!format!("{events:?}").contains("error-secret"));
}
#[test]
fn connection_no_index_alias_later_attaches_to_numeric_index() {
    let (events, _) = support::chat::run(frames(vec![
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"id":"a","function":{"name":"f","arguments":"{"}}]}}]}),
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":7,"id":"a","function":{"arguments":"\"x\":"}}]}}]}),
        serde_json::json!({"choices":[{"delta":{"tool_calls":[{"index":7,"id":"alias","function":{"arguments":"1}"}}]},"finish_reason":"tool_calls"}]}),
    ]));
    let m = terminal(&events);
    assert_eq!(m.failure, None);
    assert_eq!(m.content.len(), 1);
    let AssistantContent::ToolCall(t) = &m.content[0] else {
        panic!()
    };
    assert_eq!(t.id, "a");
    assert_eq!(t.arguments(), serde_json::json!({"x":1}).as_object());
}
#[test]
fn connection_resolves_authentication_once_across_setup_retries() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    t.responses
        .lock()
        .unwrap()
        .push_front(Err(Failure::Transport));
    t.responses
        .lock()
        .unwrap()
        .push_front(Err(Failure::SetupTimeout));
    let resolver = std::sync::Arc::new(support::auth::Resolver::new(vec![Ok(
        RequestAuth::ConfiguredWithoutSecret { source: None },
    )]));
    let out = support::block_on(models.complete(
        m,
        context(),
        support::auth::resolving(resolver.clone()),
    ));
    assert_eq!(out.failure, None);
    assert_eq!(resolver.count(), 1);
    assert_eq!(t.requests.lock().unwrap().len(), 3);
}
#[test]
fn connection_json_content_type_is_a_request_header_default() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    support::block_on(models.complete(m, context(), support::auth::local()));
    assert_eq!(
        t.requests.lock().unwrap()[0]
            .headers
            .get("content-type")
            .map(String::as_str),
        Some("application/json")
    );
}
#[test]
fn connection_foreign_projection_repair_and_image_omissions_reach_wire() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let mut c = context();
    let mut a = replay(vec![
        AssistantContent::Thinking(ThinkingContent::Readable {
            text: "foreign thinking".into(),
            signature: Some("private-signature".into()),
        }),
        AssistantContent::ToolCall(ToolCall::new(
            "id!|other".into(),
            "f".into(),
            serde_json::Map::new(),
            Some("private-signature".into()),
        )),
    ]);
    a.model = "foreign".into();
    let mut failed = a.clone();
    failed.stop_reason = Some(StopReason::Error);
    c.messages = vec![
        Message::Assistant(failed),
        Message::Assistant(a),
        Message::User(UserMessage {
            content: vec![InputContent::Image(ImageContent {
                data: "not-fetched".into(),
                mime_type: "image/png".into(),
            })],
            timestamp: 1,
        }),
    ];
    let before = c.clone();
    support::block_on(models.complete(m, c.clone(), support::auth::local()));
    assert_eq!(before, c);
    let p = payload(&t);
    assert_eq!(p["messages"].as_array().unwrap().len(), 4);
    assert_eq!(p["messages"][1]["content"], "foreign thinking");
    assert_eq!(p["messages"][1]["tool_calls"][0]["id"], "id_");
    assert_eq!(p["messages"][2]["tool_call_id"], "id_");
    assert_eq!(p["messages"][2]["content"], "No result provided");
    assert_eq!(
        p["messages"][3]["content"][0]["text"],
        "(image omitted: model does not support images)"
    );
    assert_eq!(p["tools"], serde_json::json!([]));
    assert!(
        !String::from_utf8(t.requests.lock().unwrap()[0].body.clone())
            .unwrap()
            .contains("private-signature")
    );
}
#[test]
fn connection_large_representable_retry_delay_remains_uncapped() {
    let (models, m, t) = support::chat::fixture(success(), dialect());
    let mut r = response(429, vec![]);
    r.headers.insert("retry-after-ms".into(), "1e22".into());
    t.responses.lock().unwrap().push_front(Ok(r));
    support::block_on(models.complete(m, context(), support::auth::local()));
    assert_eq!(
        t.delays.lock().unwrap()[0],
        std::time::Duration::from_secs(10000000000000000000)
    );
}
#[test]
fn connection_fallback_jitter_upper_boundary_and_large_indices() {
    let t = std::sync::Arc::new(Transport {
        jitter: f64::from_bits(0.25f64.to_bits() - 1),
        ..Transport::default()
    });
    for _ in 0..32 {
        t.responses
            .lock()
            .unwrap()
            .push_back(Err(Failure::Transport));
    }
    t.responses
        .lock()
        .unwrap()
        .push_back(Ok(response(200, vec![Ok(success())])));
    let mut models = Models::new(std::sync::Arc::new(|| 73));
    let m = support::chat::model();
    models
        .register(
            m.clone(),
            std::sync::Arc::new(ChatConnection::new(t.clone(), dialect())),
        )
        .unwrap();
    let mut o = support::auth::local();
    o.max_retries = Some(32);
    assert_eq!(
        support::block_on(models.complete(m, context(), o)).failure,
        None
    );
    let waits = t.delays.lock().unwrap();
    assert_eq!(
        &waits[..5],
        &[375, 750, 1500, 3000, 6000].map(std::time::Duration::from_millis)
    );
    assert_eq!(waits[31], std::time::Duration::from_millis(6000));
}
#[test]
fn connection_explicit_overflow_pattern_matrix() {
    for text in [
        "prompt is too long",
        "request_too_large",
        "input is too long for requested model",
        "exceeds the context window",
        "input token count abc exceeds the maximum",
        "maximum prompt length is 123",
        "reduce the length of the messages",
        "maximum context length is 123 tokens",
        "exceeds the limit of 123",
        "exceeds the available context size",
        "greater than the context length",
        "context window exceeds limit",
        "exceeded model token limit",
        "too large for model with 123 maximum context length",
        "model_context_window_exceeded",
        "prompt too long; exceeded context length",
        "prompt too long; exceeded max context length",
        "context_length_exceeded",
        "context length exceeded",
        "context_length exceeded",
        "context length_exceeded",
        "too many tokens",
        "token limit exceeded",
        "400 status code (no body)",
        "413(no body)",
    ] {
        for text in [text.to_owned(), text.to_ascii_uppercase()] {
            let (events, _) =
                support::chat::run(frames(vec![serde_json::json!({"error":{"message":text}})]));
            assert_eq!(
                terminal(&events).failure,
                Some(Failure::ContextOverflow),
                "{text}"
            );
        }
    }
    for (text, expected) in [
        ("Service unavailable: too many tokens", Failure::Overloaded),
        ("runner failed", Failure::AdapterFailed),
        (
            "maximum context length is １２ tokens",
            Failure::AdapterFailed,
        ),
        (
            "input token count\rmaximum exceeds the maximum",
            Failure::AdapterFailed,
        ),
        (
            "input token count\u{2028}exceeds the maximum",
            Failure::AdapterFailed,
        ),
        ("x400(no body)", Failure::AdapterFailed),
    ] {
        assert_eq!(
            terminal(
                &support::chat::run(frames(vec![serde_json::json!({"error":{"message":text}})])).0
            )
            .failure,
            Some(expected)
        );
    }
}
#[test]
fn connection_http_dates_reject_invalid_calendar_and_trailing_junk() {
    for (date, expected) in [
        ("Tue, 29 Feb 2000 00:00:00 GMT", 951782400000),
        ("Mon, 29 Feb 1999 00:00:00 GMT", 500),
        ("Sun, 06 Nov 1994 08:49:37 GMT junk", 500),
        ("Sun, 06 Nov 1994 25:49:37 GMT", 500),
        ("Sun, 06 Nov 1994 08:49:37 PST", 500),
    ] {
        let (models, m, t) = support::chat::fixture(success(), dialect());
        let mut r = response(429, vec![]);
        r.headers.insert("retry-after".into(), date.into());
        t.responses.lock().unwrap().push_front(Ok(r));
        support::block_on(models.complete(m, context(), support::auth::local()));
        assert_eq!(
            t.delays.lock().unwrap()[0],
            std::time::Duration::from_millis(expected)
        );
    }
}
#[test]
fn connection_id_conversion_preserves_surrogate_and_collision_boundaries() {
    let mut d = dialect();
    d.truncate_plain_call_ids = true;
    let connection = ChatConnection::new(std::sync::Arc::new(Transport::default()), d);
    let m = support::chat::model();
    let ids = [
        "abc!|x".to_string(),
        "abc?|y".into(),
        "abc__1".into(),
        format!("{}😀x", "a".repeat(39)),
    ];
    let mut source = replay(
        ids.iter()
            .map(|id| {
                AssistantContent::ToolCall(ToolCall::new(
                    id.clone(),
                    "f".into(),
                    serde_json::Map::new(),
                    None,
                ))
            })
            .collect(),
    );
    source.model = "foreign".into();
    let converted = ids
        .iter()
        .map(|id| connection.normalize_tool_call_id(id, &m, &source))
        .collect::<Vec<_>>();
    assert_eq!(
        converted,
        vec![
            "abc_".to_string(),
            "abc__2".into(),
            "abc__1".into(),
            "a".repeat(39)
        ]
    );
    source.content.reverse();
    assert_eq!(
        converted,
        ids.iter()
            .map(|id| connection.normalize_tool_call_id(id, &m, &source))
            .collect::<Vec<_>>()
    );
    assert_eq!(connection.normalize_tool_call_id("|empty", &m, &source), "");
    assert_eq!(
        connection.normalize_tool_call_id(&format!("{}😀|x", "a".repeat(39)), &m, &source),
        format!("{}_", "a".repeat(39))
    );
}
#[test]
fn connection_endpoint_percent_query_and_fragment_follow_url_construction() {
    for (endpoint, expected) in [
        (
            "http://localhost/a%2Fb",
            "http://localhost/a%2Fb/chat/completions",
        ),
        (
            "http://localhost/v1?q=x",
            "http://localhost/v1?q=x/chat/completions",
        ),
        (
            "http://localhost/v1#fragment",
            "http://localhost/v1#fragment/chat/completions",
        ),
    ] {
        let (_, mut m, t) = support::chat::fixture(success(), dialect());
        m.endpoint = endpoint.into();
        let mut models = Models::new(std::sync::Arc::new(|| 73));
        models
            .register(
                m.clone(),
                std::sync::Arc::new(ChatConnection::new(t.clone(), dialect())),
            )
            .unwrap();
        support::block_on(models.complete(m, context(), support::auth::local()));
        assert_eq!(t.requests.lock().unwrap()[0].url, expected);
    }
}
#[test]
fn connection_thinking_off_remapping_and_budget_rejection() {
    for disabled in [false, true] {
        let mut d = dialect();
        d.thinking_format = Some(ChatThinkingFormat::NestedEffort);
        let (_, mut m, t) = support::chat::fixture(success(), d.clone());
        m.capabilities.reasoning = true;
        if disabled {
            for level in [
                ThinkingLevel::Off,
                ThinkingLevel::Minimal,
                ThinkingLevel::Low,
                ThinkingLevel::Medium,
                ThinkingLevel::High,
                ThinkingLevel::Xhigh,
            ] {
                m.capabilities.thinking_level_map.insert(level, None);
            }
        }
        let mut models = Models::new(std::sync::Arc::new(|| 73));
        models
            .register(
                m.clone(),
                std::sync::Arc::new(ChatConnection::new(t.clone(), d)),
            )
            .unwrap();
        support::block_on(models.complete(m, context(), support::auth::local()));
        assert_eq!(payload(&t).get("reasoning").is_some(), !disabled);
    }
    let (_, mut m, t) = support::chat::fixture(success(), dialect());
    m.capabilities.reasoning = true;
    m.capabilities.output_limit = 16000;
    m.capabilities.thinking_mode = ThinkingMode::TokenBudget;
    let mut models = Models::new(std::sync::Arc::new(|| 73));
    models
        .register(
            m.clone(),
            std::sync::Arc::new(ChatConnection::new(t.clone(), dialect())),
        )
        .unwrap();
    let mut o = support::auth::local();
    o.thinking = ThinkingLevel::High;
    assert_eq!(
        support::block_on(models.complete(m, context(), o)).failure,
        Some(Failure::UnsupportedOperation)
    );
    assert!(t.requests.lock().unwrap().is_empty());
}
