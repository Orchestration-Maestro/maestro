mod support;
use maestro_models::*;
use std::sync::Arc;
use support::block_on;

#[test]
fn empty_success_has_exact_start_and_done() {
    let model = Model {
        identity: ModelIdentity {
            provider: "local".into(),
            model: "test".into(),
            operation: "chat".into(),
        },
        protocol: "script".into(),
        capabilities: RequestCapabilities::default(),
        rates: None,
        headers: Default::default(),
        input: vec!["text".into()],
    };
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Steps(vec![
        ScriptStep::Update(ProviderUpdate::Done {
            reason: StopReason::Stop,
        }),
    ])]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model.clone(), fake).unwrap();
    let mut stream = models.stream(
        model,
        Context {
            system_prompt: None,
            messages: vec![],
            tools: vec![],
        },
        support::auth::local(),
    );
    assert!(
        matches!(block_on(stream.next()), Some(ModelEvent::Start { partial }) if partial.content.is_empty())
    );
    assert!(
        matches!(block_on(stream.next()), Some(ModelEvent::Done { reason: StopReason::Stop, message }) if message.content.is_empty())
    );
    assert_eq!(block_on(stream.next()), None);
    assert_eq!(block_on(stream.next()), None);
}

#[test]
fn empty_and_interleaved_blocks_keep_exact_indices_and_order() {
    use support::conformance::*;
    let events = run(vec![
        ProviderUpdate::TextStart { content_index: 0 },
        ProviderUpdate::ThinkingStart {
            content_index: 1,
            signature: None,
        },
        ProviderUpdate::TextDelta {
            content_index: 0,
            delta: "".into(),
        },
        ProviderUpdate::ThinkingDelta {
            content_index: 1,
            delta: "".into(),
        },
        ProviderUpdate::TextEnd {
            content_index: 0,
            replay_metadata: None,
        },
        ProviderUpdate::ThinkingEnd { content_index: 1 },
        ProviderUpdate::TextStart { content_index: 2 },
        ProviderUpdate::ThinkingStart {
            content_index: 3,
            signature: None,
        },
        tool_start(4),
        ProviderUpdate::TextDelta {
            content_index: 2,
            delta: "answer".into(),
        },
        ProviderUpdate::ToolCallDelta {
            content_index: 4,
            delta: "{}".into(),
        },
        ProviderUpdate::ThinkingDelta {
            content_index: 3,
            delta: "reason".into(),
        },
        ProviderUpdate::ToolCallEnd { content_index: 4 },
        ProviderUpdate::ThinkingEnd { content_index: 3 },
        ProviderUpdate::TextEnd {
            content_index: 2,
            replay_metadata: None,
        },
        done(),
    ]);
    assert_eq!(
        trace(&events),
        vec![
            ("start", None),
            ("text_start", Some(0)),
            ("thinking_start", Some(1)),
            ("text_delta", Some(0)),
            ("thinking_delta", Some(1)),
            ("text_end", Some(0)),
            ("thinking_end", Some(1)),
            ("text_start", Some(2)),
            ("thinking_start", Some(3)),
            ("tool_start", Some(4)),
            ("text_delta", Some(2)),
            ("tool_delta", Some(4)),
            ("thinking_delta", Some(3)),
            ("tool_end", Some(4)),
            ("thinking_end", Some(3)),
            ("text_end", Some(2)),
            ("done", None)
        ]
    );
    assert_eq!(terminal(&events).content.len(), 5);
    assert_eq!(
        snapshot(&events[1]).content,
        vec![AssistantContent::Text(TextContent {
            text: "".into(),
            replay_metadata: None
        })]
    );
    assert_eq!(
        snapshot(&events[3]).content[0],
        AssistantContent::Text(TextContent {
            text: "".into(),
            replay_metadata: None
        })
    );
    assert_eq!(
        terminal(&events).content[2],
        AssistantContent::Text(TextContent {
            text: "answer".into(),
            replay_metadata: None
        })
    );
}

#[test]
fn readable_and_redacted_thinking_remain_distinct() {
    use support::conformance::*;
    let events = run(vec![
        ProviderUpdate::ThinkingStart {
            content_index: 0,
            signature: Some("signed".into()),
        },
        ProviderUpdate::ThinkingDelta {
            content_index: 0,
            delta: "reason".into(),
        },
        ProviderUpdate::ThinkingEnd { content_index: 0 },
        ProviderUpdate::RedactedThinking {
            content_index: 1,
            data: "opaque-secret".into(),
        },
        done(),
    ]);
    assert_eq!(
        trace(&events),
        vec![
            ("start", None),
            ("thinking_start", Some(0)),
            ("thinking_delta", Some(0)),
            ("thinking_end", Some(0)),
            ("thinking_start", Some(1)),
            ("thinking_end", Some(1)),
            ("done", None)
        ]
    );
    assert_eq!(
        snapshot(&events[1]).content[0],
        AssistantContent::Thinking(ThinkingContent::Readable {
            text: "".into(),
            signature: Some("signed".into())
        })
    );
    assert_eq!(
        terminal(&events).content,
        vec![
            AssistantContent::Thinking(ThinkingContent::Readable {
                text: "reason".into(),
                signature: Some("signed".into())
            }),
            AssistantContent::Thinking(ThinkingContent::Redacted {
                data: "opaque-secret".into()
            })
        ]
    );
    assert!(matches!(&events[5], ModelEvent::ThinkingEnd { content, .. } if content.is_empty()));
}

#[test]
fn tool_arguments_become_objects_only_at_toolcall_end() {
    use support::conformance::*;
    let events = run(vec![
        tool_start(0),
        tool_start(1),
        ProviderUpdate::ToolCallDelta {
            content_index: 0,
            delta: "{\"nested\":{\"items\":[1,true]}}".into(),
        },
        ProviderUpdate::ToolCallDelta {
            content_index: 1,
            delta: "{}".into(),
        },
        ProviderUpdate::ToolCallEnd { content_index: 1 },
        ProviderUpdate::ToolCallEnd { content_index: 0 },
        ProviderUpdate::Done {
            reason: StopReason::ToolUse,
        },
    ]);
    for event in &events[1..5] {
        for content in &snapshot(event).content {
            if let AssistantContent::ToolCall(call) = content {
                assert_eq!(call.arguments(), None);
            }
        }
    }
    for (index, expected) in [
        (0, serde_json::json!({"nested":{"items":[1,true]}})),
        (1, serde_json::json!({})),
    ] {
        let AssistantContent::ToolCall(call) = &terminal(&events).content[index] else {
            panic!("missing call")
        };
        assert_eq!(call.id, format!("call-{index}"));
        assert_eq!(call.name, "lookup");
        assert_eq!(call.replay_metadata.as_deref(), Some("opaque"));
        assert_eq!(call.arguments(), expected.as_object());
    }
    assert!(
        matches!(&events[5], ModelEvent::ToolCallEnd { tool_call, .. } if tool_call.id == "call-1" && tool_call.arguments().unwrap().is_empty())
    );
}

#[test]
fn unicode_and_json_fragments_reassemble_without_loss() {
    use support::conformance::*;
    let mut updates = vec![
        ProviderUpdate::TextStart { content_index: 0 },
        ProviderUpdate::ThinkingStart {
            content_index: 1,
            signature: None,
        },
        tool_start(2),
    ];
    for chunk in ["é", "e", "\u{301}", "👩", "\u{200d}", "💻", "漢字"] {
        updates.push(ProviderUpdate::TextDelta {
            content_index: 0,
            delta: chunk.into(),
        });
        updates.push(ProviderUpdate::ThinkingDelta {
            content_index: 1,
            delta: chunk.into(),
        });
    }
    for chunk in ["{\"v\":\"", "\\uD83", "D\\u", "DE00", "\\", "n\"}"] {
        updates.push(ProviderUpdate::ToolCallDelta {
            content_index: 2,
            delta: chunk.into(),
        });
    }
    updates.extend([
        ProviderUpdate::ToolCallEnd { content_index: 2 },
        ProviderUpdate::TextEnd {
            content_index: 0,
            replay_metadata: None,
        },
        ProviderUpdate::ThinkingEnd { content_index: 1 },
        done(),
    ]);
    let events = run(updates);
    assert_eq!(
        terminal(&events).content[0],
        AssistantContent::Text(TextContent {
            text: "ée\u{301}👩\u{200d}💻漢字".into(),
            replay_metadata: None
        })
    );
    assert_eq!(
        terminal(&events).content[1],
        AssistantContent::Thinking(ThinkingContent::Readable {
            text: "ée\u{301}👩\u{200d}💻漢字".into(),
            signature: None
        })
    );
    let AssistantContent::ToolCall(call) = &terminal(&events).content[2] else {
        panic!("missing tool")
    };
    assert_eq!(
        call.arguments(),
        serde_json::json!({"v":"😀\n"}).as_object()
    );
    for json in [
        "{\"v\":\"\\uD800\"}",
        "{\"v\":\"\\uDC00\"}",
        "{\"v\":\"\\q\"}",
    ] {
        let events = run(vec![
            tool_start(0),
            ProviderUpdate::ToolCallDelta {
                content_index: 0,
                delta: json.into(),
            },
            ProviderUpdate::ToolCallEnd { content_index: 0 },
            done(),
        ]);
        assert_eq!(terminal(&events).failure, Some(Failure::MalformedStream));
    }
}

#[test]
fn success_and_failure_reasons_are_exact_and_terminal() {
    use support::conformance::*;
    for reason in [
        StopReason::Stop,
        StopReason::Length,
        StopReason::ToolUse,
        StopReason::Error,
        StopReason::Aborted,
    ] {
        let update = match reason {
            StopReason::Stop | StopReason::Length | StopReason::ToolUse => {
                ProviderUpdate::Done { reason }
            }
            StopReason::Error => ProviderUpdate::Error {
                failure: Failure::AdapterFailed,
            },
            StopReason::Aborted => ProviderUpdate::Error {
                failure: Failure::Cancelled,
            },
        };
        let events = run(vec![update, done()]);
        assert_eq!(terminal(&events).stop_reason, Some(reason));
        match events.last().unwrap() {
            ModelEvent::Done { reason: actual, .. }
                if matches!(
                    reason,
                    StopReason::Stop | StopReason::Length | StopReason::ToolUse
                ) =>
            {
                assert_eq!(*actual, reason)
            }
            ModelEvent::Error { reason: actual, .. }
                if matches!(reason, StopReason::Error | StopReason::Aborted) =>
            {
                assert_eq!(*actual, reason)
            }
            _ => panic!("wrong terminal family"),
        }
    }
    for reason in [StopReason::Error, StopReason::Aborted] {
        let events = run(vec![ProviderUpdate::Done { reason }]);
        assert_eq!(terminal(&events).failure, Some(Failure::MalformedStream));
    }
}

#[test]
fn retained_snapshots_own_nested_content_usage_and_identity() {
    use support::conformance::*;
    let mut updates = text(0, "early");
    updates.pop();
    updates.extend([
        ProviderUpdate::Usage {
            usage: flat_usage(),
        },
        ProviderUpdate::ResponseIdentity {
            response_model: Some("actual-a".into()),
            response_id: Some("id-a".into()),
        },
        ProviderUpdate::ThinkingStart {
            content_index: 1,
            signature: Some("signature".into()),
        },
        ProviderUpdate::ThinkingDelta {
            content_index: 1,
            delta: "first".into(),
        },
        tool_start(2),
        ProviderUpdate::ToolCallDelta {
            content_index: 2,
            delta: "{\"nested\":{\"items\":[1]}}".into(),
        },
        ProviderUpdate::ToolCallEnd { content_index: 2 },
        ProviderUpdate::Usage {
            usage: Usage {
                output: 99,
                ..usage()
            },
        },
        ProviderUpdate::ResponseIdentity {
            response_model: Some("actual-b".into()),
            response_id: Some("id-b".into()),
        },
        ProviderUpdate::TextDelta {
            content_index: 0,
            delta: "later".into(),
        },
        ProviderUpdate::ThinkingDelta {
            content_index: 1,
            delta: "second".into(),
        },
        ProviderUpdate::ThinkingEnd { content_index: 1 },
        ProviderUpdate::TextEnd {
            content_index: 0,
            replay_metadata: None,
        },
        done(),
    ]);
    let fake = Arc::new(ScriptedProvider::new(vec![steps(updates)]));
    let models = priced_registry(fake);
    let mut stream = models.stream(model(), context(), support::auth::local());
    let mut retained = vec![];
    for _ in 0..9 {
        retained.push(block_on(stream.next()).unwrap());
    }
    let frozen = retained.clone();
    while let Some(event) = block_on(stream.next()) {
        retained.push(event);
    }
    assert_contract(&retained, &model(), 73);
    assert_eq!(&retained[..9], frozen.as_slice());
    assert!(snapshot(&retained[0]).content.is_empty());
    assert_eq!(
        snapshot(&retained[1]).content[0],
        AssistantContent::Text(TextContent {
            text: "".into(),
            replay_metadata: None
        })
    );
    assert_eq!(
        snapshot(&retained[2]).content[0],
        AssistantContent::Text(TextContent {
            text: "early".into(),
            replay_metadata: None
        })
    );
    assert_eq!(snapshot(&retained[2]).usage, Usage::default());
    assert_accounting(
        &snapshot(&retained[3]).usage,
        &Usage {
            cost: UsageCost {
                input: 0.000022,
                output: 0.000056,
                cache_read: 0.000003,
                cache_write: 0.000008,
                total: 0.000089,
                priced: true,
            },
            ..usage()
        },
    );
    assert_eq!(
        snapshot(&retained[3]).response_model.as_deref(),
        Some("actual-a")
    );
    assert_eq!(
        snapshot(&retained[4]).content[1],
        AssistantContent::Thinking(ThinkingContent::Readable {
            text: "first".into(),
            signature: Some("signature".into())
        })
    );
    let AssistantContent::ToolCall(call) = &snapshot(&retained[6]).content[2] else {
        panic!("tool")
    };
    assert!(call.arguments().is_none());
    let mut cloned = snapshot(&retained[7]).clone();
    let AssistantContent::ToolCall(call) = &mut cloned.content[2] else {
        panic!("tool")
    };
    call.id = "mutated".into();
    let mut object = call.arguments().unwrap().clone();
    object.get_mut("nested").unwrap()["items"][0] = serde_json::json!(999);
    cloned.usage.cost.total = 999.0;
    assert_eq!(retained[7], frozen[7]);
    let AssistantContent::ToolCall(original) = &snapshot(&retained[7]).content[2] else {
        panic!("tool")
    };
    assert_eq!(
        original.arguments(),
        serde_json::json!({"nested":{"items":[1]}}).as_object()
    );
    assert_eq!(
        terminal(&retained).response_model.as_deref(),
        Some("actual-b")
    );
    assert_eq!(terminal(&retained).usage.output, 99);
    assert_accounting(
        &terminal(&retained).usage,
        &Usage {
            output: 99,
            total_tokens: 115,
            cost: UsageCost {
                input: 0.000022,
                output: 0.000792,
                cache_read: 0.000003,
                cache_write: 0.000008,
                total: 0.000825,
                priced: true,
            },
            ..usage()
        },
    );
}

#[test]
fn setup_and_transport_failures_preserve_the_normal_error_contract() {
    use support::conformance::*;
    let fake = Arc::new(ScriptedProvider::new(vec![Script::SetupFailure(
        Failure::AdapterFailed,
    )]));
    let mut direct = DirectProvider::new(vec![]);
    direct.setup_failure = Some(Failure::Transport);
    for (provider, failure) in [
        (fake as Arc<dyn Provider>, Failure::AdapterFailed),
        (Arc::new(direct) as Arc<dyn Provider>, Failure::Transport),
    ] {
        let events = collect(
            registry(provider).stream(model(), context(), support::auth::local()),
            &model(),
            73,
        );
        assert_eq!(trace(&events), vec![("error", None)]);
        assert_eq!(terminal(&events).failure, Some(failure));
        assert!(!failure.to_string().contains("synthetic secret"));
    }
    let events = run(vec![
        ProviderUpdate::TextStart { content_index: 0 },
        ProviderUpdate::TextDelta {
            content_index: 0,
            delta: "partial".into(),
        },
        ProviderUpdate::Usage {
            usage: flat_usage(),
        },
        ProviderUpdate::ResponseIdentity {
            response_model: Some("actual".into()),
            response_id: Some("response".into()),
        },
        ProviderUpdate::Error {
            failure: Failure::Transport,
        },
    ]);
    assert_eq!(terminal(&events).failure, Some(Failure::Transport));
    assert_eq!(terminal(&events).usage, usage());
    assert_eq!(terminal(&events).response_model.as_deref(), Some("actual"));
    assert_eq!(
        terminal(&events).content[0],
        AssistantContent::Text(TextContent {
            text: "partial".into(),
            replay_metadata: None
        })
    );
}

#[test]
fn malformed_blocks_and_tool_json_fail_without_repair() {
    use support::conformance::*;
    let mut cases = vec![
        vec![ProviderUpdate::TextStart { content_index: 1 }],
        vec![ProviderUpdate::TextDelta {
            content_index: 0,
            delta: "bad".into(),
        }],
        vec![ProviderUpdate::TextEnd {
            content_index: 0,
            replay_metadata: None,
        }],
        vec![ProviderUpdate::ThinkingDelta {
            content_index: 9,
            delta: "bad".into(),
        }],
        vec![ProviderUpdate::ThinkingEnd { content_index: 9 }],
        vec![ProviderUpdate::ToolCallDelta {
            content_index: 9,
            delta: "{}".into(),
        }],
        vec![ProviderUpdate::ToolCallEnd { content_index: 9 }],
        vec![
            ProviderUpdate::TextStart { content_index: 0 },
            ProviderUpdate::TextStart { content_index: 0 },
        ],
        vec![
            ProviderUpdate::TextStart { content_index: 0 },
            ProviderUpdate::ThinkingDelta {
                content_index: 0,
                delta: "bad".into(),
            },
        ],
        vec![
            ProviderUpdate::ThinkingStart {
                content_index: 0,
                signature: None,
            },
            ProviderUpdate::TextDelta {
                content_index: 0,
                delta: "bad".into(),
            },
        ],
        vec![
            ProviderUpdate::TextStart { content_index: 0 },
            ProviderUpdate::ToolCallEnd { content_index: 0 },
        ],
        vec![
            ProviderUpdate::ThinkingStart {
                content_index: 0,
                signature: None,
            },
            ProviderUpdate::TextEnd {
                content_index: 0,
                replay_metadata: None,
            },
        ],
        vec![ProviderUpdate::TextStart { content_index: 0 }, done()],
        vec![
            ProviderUpdate::ThinkingStart {
                content_index: 0,
                signature: None,
            },
            done(),
        ],
        vec![tool_start(0), done()],
        vec![
            ProviderUpdate::RedactedThinking {
                content_index: 0,
                data: "opaque".into(),
            },
            ProviderUpdate::ThinkingDelta {
                content_index: 0,
                delta: "bad".into(),
            },
        ],
    ];
    for (start, end, delta) in [
        (
            ProviderUpdate::TextStart { content_index: 0 },
            ProviderUpdate::TextEnd {
                content_index: 0,
                replay_metadata: None,
            },
            ProviderUpdate::TextDelta {
                content_index: 0,
                delta: "late".into(),
            },
        ),
        (
            ProviderUpdate::ThinkingStart {
                content_index: 0,
                signature: None,
            },
            ProviderUpdate::ThinkingEnd { content_index: 0 },
            ProviderUpdate::ThinkingDelta {
                content_index: 0,
                delta: "late".into(),
            },
        ),
    ] {
        cases.push(vec![start.clone(), end.clone(), end.clone()]);
        cases.push(vec![start.clone(), end.clone(), delta]);
        cases.push(vec![start.clone(), end, start]);
    }
    for json in [
        "",
        "{",
        "{\"secret\":",
        "{\"synthetic secret\":!}",
        "1",
        "[]",
        "null",
        "\"string\"",
        "true",
        "{} trailing",
    ] {
        cases.push(vec![
            tool_start(0),
            ProviderUpdate::ToolCallDelta {
                content_index: 0,
                delta: json.into(),
            },
            ProviderUpdate::ToolCallEnd { content_index: 0 },
        ]);
    }
    for tail in [
        ProviderUpdate::ToolCallEnd { content_index: 0 },
        ProviderUpdate::ToolCallDelta {
            content_index: 0,
            delta: "late".into(),
        },
        tool_start(0),
    ] {
        cases.push(vec![
            tool_start(0),
            ProviderUpdate::ToolCallDelta {
                content_index: 0,
                delta: "{}".into(),
            },
            ProviderUpdate::ToolCallEnd { content_index: 0 },
            tail,
        ]);
    }
    for updates in cases {
        let events = run(updates);
        assert_eq!(terminal(&events).failure, Some(Failure::MalformedStream));
        assert_eq!(
            terminal(&events).failure.unwrap().to_string(),
            "malformed provider stream"
        );
        if let Some(AssistantContent::ToolCall(call)) = terminal(&events).content.first() {
            let ended = events
                .iter()
                .any(|e| matches!(e, ModelEvent::ToolCallEnd { .. }));
            assert_eq!(call.arguments().is_some(), ended);
        }
    }
}

#[test]
fn eof_without_terminal_is_incomplete_not_success() {
    use support::conformance::*;
    for updates in [
        vec![],
        vec![ProviderUpdate::Usage {
            usage: flat_usage(),
        }],
        vec![
            ProviderUpdate::TextStart { content_index: 0 },
            ProviderUpdate::TextDelta {
                content_index: 0,
                delta: "cut".into(),
            },
        ],
        text(0, "closed"),
    ] {
        let expected_usage = if matches!(updates.first(), Some(ProviderUpdate::Usage { .. })) {
            usage()
        } else {
            Usage::default()
        };
        let events = run(updates);
        assert_eq!(terminal(&events).failure, Some(Failure::IncompleteStream));
        assert_eq!(terminal(&events).usage, expected_usage);
        if let Some(AssistantContent::Text(content)) = terminal(&events).content.first() {
            assert!(content.text == "cut" || content.text == "closed");
        }
    }
}

#[test]
fn terminal_delivery_drops_source_and_suppresses_later_updates() {
    use std::sync::atomic::Ordering;
    use support::conformance::*;
    for terminal_update in [
        done(),
        ProviderUpdate::Error {
            failure: Failure::Transport,
        },
        ProviderUpdate::Error {
            failure: Failure::Cancelled,
        },
    ] {
        let updates = vec![
            terminal_update.clone(),
            ProviderUpdate::TextStart { content_index: 0 },
        ];
        let scripted = run(updates.clone());
        assert!(terminal(&scripted).content.is_empty());
        let direct = Arc::new(DirectProvider::new(updates));
        let models = registry(direct.clone());
        let options = support::auth::local();
        let mut stream = models.stream(model(), context(), options.clone());
        while let Some(event) = block_on(stream.next()) {
            if matches!(event, ModelEvent::Done { .. } | ModelEvent::Error { .. }) {
                break;
            }
        }
        assert_eq!(direct.drops.load(Ordering::SeqCst), 1);
        assert_eq!(direct.polls.load(Ordering::SeqCst), 1);
        options.cancellation.cancel();
        assert_eq!(block_on(stream.next()), None);
        assert_eq!(block_on(stream.next()), None);
        assert_eq!(direct.polls.load(Ordering::SeqCst), 1);
        assert_eq!(direct.drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn reported_usage_and_response_identity_survive_failure() {
    use support::conformance::*;
    for outcome in [
        done(),
        ProviderUpdate::Error {
            failure: Failure::Transport,
        },
        ProviderUpdate::Error {
            failure: Failure::Cancelled,
        },
    ] {
        let mut updates = text(0, "answer");
        updates.extend([
            ProviderUpdate::Usage {
                usage: Usage {
                    input: 1,
                    ..Usage::default()
                },
            },
            ProviderUpdate::ResponseIdentity {
                response_model: Some("first".into()),
                response_id: None,
            },
            ProviderUpdate::TextStart { content_index: 1 },
            ProviderUpdate::TextEnd {
                content_index: 1,
                replay_metadata: None,
            },
            ProviderUpdate::Usage {
                usage: flat_usage(),
            },
            ProviderUpdate::ResponseIdentity {
                response_model: Some("actual".into()),
                response_id: Some("id".into()),
            },
            outcome,
        ]);
        let events = run(updates);
        let final_record = terminal(&events);
        assert_eq!(final_record.usage, usage());
        assert_eq!(final_record.response_model.as_deref(), Some("actual"));
        assert_eq!(final_record.response_id.as_deref(), Some("id"));
        assert_eq!(final_record.model, "test");
        assert_eq!(snapshot(&events[1]).usage, Usage::default());
        assert_eq!(snapshot(&events[1]).response_model, None);
        assert_eq!(snapshot(&events[4]).usage.input, 1);
        assert_eq!(
            snapshot(&events[4]).response_model.as_deref(),
            Some("first")
        );
        assert_eq!(snapshot(&events[4]).response_id, None);
    }
    assert_eq!(terminal(&run(vec![done()])).usage, Usage::default());
}

#[test]
fn text_replay_metadata_survives_stream_completion() {
    use support::conformance::*;
    let updates = vec![
        ProviderUpdate::TextStart { content_index: 0 },
        ProviderUpdate::TextDelta {
            content_index: 0,
            delta: "answer".into(),
        },
        ProviderUpdate::TextEnd {
            content_index: 0,
            replay_metadata: Some("opaque-signature".into()),
        },
        done(),
    ];
    let fake = Arc::new(ScriptedProvider::new(vec![
        steps(updates.clone()),
        steps(updates),
    ]));
    let models = registry(fake);
    let events = collect(
        models.stream(model(), context(), support::auth::local()),
        &model(),
        73,
    );
    assert_eq!(
        trace(&events),
        vec![
            ("start", None),
            ("text_start", Some(0)),
            ("text_delta", Some(0)),
            ("text_end", Some(0)),
            ("done", None)
        ]
    );
    let unsigned = AssistantContent::Text(TextContent {
        text: "answer".into(),
        replay_metadata: None,
    });
    let signed = AssistantContent::Text(TextContent {
        text: "answer".into(),
        replay_metadata: Some("opaque-signature".into()),
    });
    assert_eq!(snapshot(&events[2]).content, vec![unsigned.clone()]);
    assert_eq!(snapshot(&events[3]).content, vec![signed.clone()]);
    assert_eq!(terminal(&events).content, vec![signed]);
    let completed = block_on(models.complete(model(), context(), support::auth::local()));
    assert_eq!(&completed, terminal(&events));
    let source = Context {
        messages: vec![Message::Assistant(completed.clone())],
        ..context()
    };
    assert_eq!(
        project_context(&source, &model(), &|_, _, _| panic!(), 73),
        source
    );
    let mut foreign = model();
    foreign.identity.model = "foreign".into();
    let projected = project_context(&source, &foreign, &|_, _, _| panic!(), 73);
    let Message::Assistant(a) = &projected.messages[0] else {
        panic!()
    };
    assert_eq!(a.content, vec![unsigned]);
    assert_eq!(
        snapshot(&events[2]).content[0],
        AssistantContent::Text(TextContent {
            text: "answer".into(),
            replay_metadata: None
        })
    );
}
