mod support;
use maestro_models::*;
use serde_json::json;
use std::sync::Arc;
use support::{block_on, conformance::*};

fn input(text: &str) -> InputContent {
    InputContent::Text(TextContent {
        text: text.into(),
        replay_metadata: None,
    })
}
fn image(data: &str) -> InputContent {
    InputContent::Image(ImageContent {
        data: data.into(),
        mime_type: "image/png".into(),
    })
}
fn declaration() -> ToolDeclaration {
    ToolDeclaration {
        name: "lookup".into(),
        description: "Look up data".into(),
        parameters: json!({"type":"object"}),
    }
}
fn result(id: &str) -> Message {
    Message::ToolResult(ToolResultMessage {
        tool_call_id: id.into(),
        tool_name: "lookup".into(),
        content: vec![input("found")],
        details: Some(json!({"private":"sentinel"})),
        is_error: false,
        timestamp: 19,
    })
}

#[test]
fn conversation_records_round_trip_through_model_interface() {
    let mut target = model();
    target.input = vec!["text".into(), "image".into()];
    let original = Context {
        system_prompt: Some("current".into()),
        messages: vec![Message::User(UserMessage {
            content: vec![input("hello"), image("AQID")],
            timestamp: 17,
        })],
        tools: vec![declaration()],
    };
    let mut updates = text(0, "answer");
    updates.extend([
        tool_start(1),
        ProviderUpdate::ToolCallDelta {
            content_index: 1,
            delta: "{\"nested\":{\"x\":1}}".into(),
        },
        ProviderUpdate::ToolCallEnd { content_index: 1 },
        ProviderUpdate::Usage { usage: usage() },
        ProviderUpdate::ResponseIdentity {
            response_model: Some("actual".into()),
            response_id: Some("response".into()),
        },
        ProviderUpdate::Done {
            reason: StopReason::ToolUse,
        },
    ]);
    let fake = Arc::new(ScriptedProvider::new(vec![
        steps(updates),
        steps(vec![done()]),
    ]));
    let mut models = Models::new(Arc::new(|| 73));
    models.register(target.clone(), fake.clone()).unwrap();
    let events = collect(
        models.stream(target.clone(), original.clone(), support::auth::local()),
        &target,
        73,
    );
    let assistant = terminal(&events).clone();
    assert_eq!(assistant.usage, usage());
    assert_eq!(assistant.timestamp, 73);
    assert_eq!(assistant.response_model.as_deref(), Some("actual"));
    assert_eq!(assistant.response_id.as_deref(), Some("response"));
    assert_eq!(
        (&assistant.provider, &assistant.protocol, &assistant.model),
        (
            &target.identity.provider,
            &target.protocol,
            &target.identity.model
        )
    );
    let AssistantContent::ToolCall(call) = &assistant.content[1] else {
        panic!()
    };
    assert_eq!((&call.id[..], &call.name[..]), ("call-1", "lookup"));
    assert_eq!(call.arguments(), json!({"nested":{"x":1}}).as_object());
    let mut history = original.clone();
    history
        .messages
        .extend([Message::Assistant(assistant.clone()), result("call-1")]);
    let before = history.clone();
    block_on(models.complete(target, history.clone(), support::auth::local()));
    assert_eq!(history, before);
    assert_eq!(fake.calls()[0].context, original);
    let mut expected = history.clone();
    let Message::ToolResult(r) = &mut expected.messages[2] else {
        panic!()
    };
    r.details = None;
    assert_eq!(fake.calls()[1].context, expected);
    for message in &history.messages {
        match message {
            Message::User(u) => assert_eq!(u.timestamp, 17),
            Message::Assistant(a) => assert_eq!(a, &assistant),
            Message::ToolResult(r) => {
                assert_eq!(r.timestamp, 19);
                assert!(!r.is_error);
                assert_eq!(r.details, Some(json!({"private":"sentinel"})));
            }
        }
    }
}

#[test]
fn current_prompt_and_tools_remain_separate_request_inputs() {
    let source = context();
    let before = source.clone();
    let mut changed = source.clone();
    changed.system_prompt = Some("replacement prompt".into());
    changed.tools = vec![declaration()];
    let expected = [source.clone(), changed.clone()];
    let scripts = expected
        .into_iter()
        .map(|expected| {
            Script::Factory(Box::new(move |call| {
                Box::pin(async move {
                    assert_eq!(call.context, expected);
                    Ok(vec![ScriptStep::Update(done())])
                })
            }))
        })
        .collect();
    let fake = Arc::new(ScriptedProvider::new(scripts));
    let models = registry(fake.clone());
    for input in [source.clone(), changed] {
        block_on(models.complete(model(), input, support::auth::local()));
    }
    assert_eq!(source, before);
    assert_eq!(
        fake.calls()[0].context.messages,
        fake.calls()[1].context.messages
    );
    assert_eq!(fake.calls().len(), 2);
}

#[test]
fn tool_details_and_execution_policy_never_enter_provider_context() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let executions = AtomicUsize::new(0);
    let execute = || {
        executions.fetch_add(1, Ordering::SeqCst);
    };
    let mut source = context();
    source.tools.push(declaration());
    source.messages.push(result("caller-call"));
    let before = source.clone();
    let expected_tools = source.tools.clone();
    let fake = Arc::new(ScriptedProvider::new(vec![Script::Factory(Box::new(
        move |call| {
            Box::pin(async move {
                assert_eq!(call.context.tools, expected_tools);
                let Message::ToolResult(result) = &call.context.messages[1] else {
                    panic!()
                };
                assert_eq!(result.details, None);
                assert_eq!(result.content, vec![input("found")]);
                Ok(vec![ScriptStep::Update(done())])
            })
        },
    ))]));
    block_on(registry(fake).complete(model(), source.clone(), support::auth::local()));
    assert_eq!(source, before);
    assert_eq!(executions.load(Ordering::SeqCst), 0);
    execute();
    assert_eq!(executions.load(Ordering::SeqCst), 1);
}

fn assistant(blocks: Vec<AssistantContent>) -> AssistantMessage {
    AssistantMessage {
        provider: model().identity.provider,
        protocol: model().protocol,
        model: model().identity.model,
        timestamp: 11,
        content: blocks,
        usage: usage(),
        stop_reason: Some(StopReason::ToolUse),
        failure: None,
        response_model: Some("different-actual".into()),
        response_id: Some("response".into()),
    }
}
fn call(id: &str) -> AssistantContent {
    AssistantContent::ToolCall(ToolCall::new(
        id.into(),
        "lookup".into(),
        json!({"x":1}).as_object().unwrap().clone(),
        Some("call-signature".into()),
    ))
}
fn replay_context() -> Context {
    Context {
        system_prompt: Some("prompt".into()),
        tools: vec![declaration()],
        messages: vec![
            Message::Assistant(assistant(vec![
                AssistantContent::Text(TextContent {
                    text: "answer".into(),
                    replay_metadata: Some("text-signature".into()),
                }),
                AssistantContent::Thinking(ThinkingContent::Readable {
                    text: "reason".into(),
                    signature: Some("thinking-signature".into()),
                }),
                AssistantContent::Thinking(ThinkingContent::Readable {
                    text: "".into(),
                    signature: Some("empty-signature".into()),
                }),
                AssistantContent::Thinking(ThinkingContent::Readable {
                    text: "  \n".into(),
                    signature: None,
                }),
                AssistantContent::Thinking(ThinkingContent::Redacted {
                    data: "opaque-redaction".into(),
                }),
                call("original"),
            ])),
            result("original"),
        ],
    }
}

#[test]
fn same_model_replay_retains_opaque_metadata() {
    let source = replay_context();
    let mut expected = source.clone();
    let Message::Assistant(a) = &mut expected.messages[0] else {
        panic!()
    };
    a.content.remove(3);
    let Message::ToolResult(r) = &mut expected.messages[1] else {
        panic!()
    };
    r.details = None;
    assert_eq!(
        project_context(
            &source,
            &model(),
            &|_, _, _| panic!("same model must not normalize IDs"),
            73
        ),
        expected
    );
}

#[test]
fn foreign_replay_drops_signatures_and_redaction() {
    let source = replay_context();
    for change in 0..3 {
        let mut target = model();
        match change {
            0 => target.identity.provider = "foreign".into(),
            1 => target.protocol = "foreign".into(),
            _ => target.identity.model = "foreign".into(),
        }
        let projected = project_context(&source, &target, &|id, _, _| id.to_owned(), 73);
        let Message::Assistant(a) = &projected.messages[0] else {
            panic!()
        };
        assert_eq!(
            a.content,
            vec![
                AssistantContent::Text(TextContent {
                    text: "answer".into(),
                    replay_metadata: None
                }),
                AssistantContent::Text(TextContent {
                    text: "reason".into(),
                    replay_metadata: None
                }),
                AssistantContent::ToolCall(ToolCall::new(
                    "original".into(),
                    "lookup".into(),
                    json!({"x":1}).as_object().unwrap().clone(),
                    None
                ))
            ]
        );
        assert_eq!(a.timestamp, 11);
        assert_eq!(a.usage, usage());
        assert_eq!(source, replay_context());
    }
}

#[test]
fn call_id_rewrites_match_real_and_synthetic_results() {
    use std::cell::RefCell;
    let mut source = context();
    let mut foreign = assistant(vec![call("a"), call("b")]);
    foreign.provider = "foreign".into();
    source.messages.extend([
        Message::Assistant(foreign.clone()),
        result("a"),
        Message::Assistant(assistant(vec![call("a")])),
        result("a"),
    ]);
    let seen = RefCell::new(vec![]);
    let projected = project_context(
        &source,
        &model(),
        &|id, target, origin| {
            assert_eq!(target, &model());
            assert_eq!(origin, &foreign);
            seen.borrow_mut().push(id.to_owned());
            format!("new-{id}")
        },
        73,
    );
    assert_eq!(*seen.borrow(), vec!["a", "b"]);
    let ids: Vec<_> = projected
        .messages
        .iter()
        .filter_map(|m| {
            if let Message::ToolResult(r) = m {
                Some(r.tool_call_id.as_str())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(ids, vec!["new-a", "new-b", "a"]);
    let Message::Assistant(first) = &projected.messages[1] else {
        panic!()
    };
    let AssistantContent::ToolCall(c) = &first.content[0] else {
        panic!()
    };
    assert_eq!(c.id, "new-a");
    assert_eq!(c.name, "lookup");
    assert_eq!(c.arguments(), json!({"x":1}).as_object());
    let Message::ToolResult(synthetic) = &projected.messages[3] else {
        panic!()
    };
    assert_eq!(synthetic.content, vec![input("No result provided")]);
    assert_eq!(synthetic.timestamp, 73);
    assert!(synthetic.is_error);
    assert_eq!(source.messages[1], Message::Assistant(foreign));
}

#[test]
fn missing_results_are_completed_at_each_turn_boundary() {
    let mut failed = assistant(vec![call("ignored")]);
    failed.stop_reason = Some(StopReason::Error);
    failed.failure = Some(Failure::Transport);
    let boundaries = [
        None,
        Some(Message::User(UserMessage {
            content: vec![input("interrupt")],
            timestamp: 22,
        })),
        Some(Message::Assistant(assistant(vec![]))),
        Some(Message::Assistant(failed)),
    ];
    for boundary in boundaries {
        for answered in 0..=2 {
            let mut source = context();
            source
                .messages
                .push(Message::Assistant(assistant(vec![call("a"), call("b")])));
            for id in ["a", "b"].into_iter().take(answered) {
                source.messages.push(result(id));
            }
            if let Some(boundary) = &boundary {
                source.messages.push(boundary.clone());
            }
            let before = source.clone();
            let projected = project_context(&source, &model(), &|_, _, _| panic!(), 99);
            let missing: Vec<_> = projected
                .messages
                .iter()
                .filter_map(|m| match m {
                    Message::ToolResult(r) if r.is_error => Some(r),
                    _ => None,
                })
                .collect();
            assert_eq!(
                missing
                    .iter()
                    .map(|r| r.tool_call_id.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b"][answered..]
            );
            for r in missing {
                assert_eq!(r.tool_name, "lookup");
                assert_eq!(r.content, vec![input("No result provided")]);
                assert_eq!(r.details, None);
                assert_eq!(r.timestamp, 99);
            }
            assert_eq!(source, before);
            assert_eq!(
                project_context(&projected, &model(), &|_, _, _| panic!(), 99),
                projected
            );
        }
    }
}

#[test]
fn failed_and_aborted_attempts_never_replay_as_completed_turns() {
    let events = run(vec![
        tool_start(0),
        ProviderUpdate::ToolCallDelta {
            content_index: 0,
            delta: "{\"partial\":".into(),
        },
        ProviderUpdate::Error {
            failure: Failure::Transport,
        },
    ]);
    let partial_call = terminal(&events).content[0].clone();
    let mut source = context();
    source.messages.extend([
        Message::Assistant(assistant(vec![call("good")])),
        result("good"),
    ]);
    let expected_source = source.clone();
    for reason in [Some(StopReason::Error), Some(StopReason::Aborted), None] {
        let mut attempt = assistant(vec![
            AssistantContent::Text(TextContent {
                text: "partial".into(),
                replay_metadata: None,
            }),
            partial_call.clone(),
        ]);
        attempt.stop_reason = reason;
        attempt.failure = Some(Failure::Transport);
        source.messages.push(Message::Assistant(attempt));
    }
    let before = source.clone();
    let mut expected = expected_source;
    let Message::ToolResult(r) = &mut expected.messages[2] else {
        panic!()
    };
    r.details = None;
    assert_eq!(
        project_context(&source, &model(), &|_, _, _| panic!(), 73),
        expected
    );
    assert_eq!(source, before);
    let AssistantContent::ToolCall(c) = &partial_call else {
        panic!()
    };
    assert_eq!(c.arguments(), None);
}

#[test]
fn unsupported_images_become_explicit_omissions() {
    for blocks in [
        vec![image("AQID")],
        vec![image("AQID"), image("BAUG")],
        vec![
            input("before"),
            image("AQID"),
            image("BAUG"),
            input("between"),
            image("BwgJ"),
            input("after"),
        ],
    ] {
        let mut source = context();
        source.messages = vec![
            Message::User(UserMessage {
                content: blocks.clone(),
                timestamp: 1,
            }),
            Message::ToolResult(ToolResultMessage {
                content: blocks.clone(),
                ..match result("id") {
                    Message::ToolResult(r) => r,
                    _ => unreachable!(),
                }
            }),
        ];
        let projected = project_context(&source, &model(), &|id, _, _| id.into(), 73);
        for (index, placeholder) in [
            (0, "(image omitted: model does not support images)"),
            (1, "(tool image omitted: model does not support images)"),
        ] {
            let content = match &projected.messages[index] {
                Message::User(u) => &u.content,
                Message::ToolResult(r) => &r.content,
                _ => panic!(),
            };
            let expected = if blocks.len() <= 2 {
                vec![input(placeholder)]
            } else {
                vec![
                    input("before"),
                    input(placeholder),
                    input("between"),
                    input(placeholder),
                    input("after"),
                ]
            };
            assert_eq!(content, &expected);
        }
        assert_eq!(
            project_context(&projected, &model(), &|id, _, _| id.into(), 73),
            projected
        );
        assert_eq!(
            match &source.messages[0] {
                Message::User(u) => &u.content,
                _ => panic!(),
            },
            &blocks
        );
    }
    let source = Context {
        messages: vec![Message::User(UserMessage {
            content: vec![
                input("(image omitted: model does not support images)"),
                image("AA=="),
            ],
            timestamp: 1,
        })],
        ..context()
    };
    let projected = project_context(&source, &model(), &|id, _, _| id.into(), 73);
    let Message::User(u) = &projected.messages[0] else {
        panic!()
    };
    assert_eq!(
        u.content,
        vec![input("(image omitted: model does not support images)")]
    );
}

#[test]
fn supported_images_keep_exact_base64_and_mime() {
    let mut target = model();
    target.input.push("image".into());
    let blocks = vec![
        image("AAECA/+/=="),
        input("middle"),
        InputContent::Image(ImageContent {
            data: "exact:opaque-base64".into(),
            mime_type: "image/custom+binary".into(),
        }),
    ];
    let source = Context {
        messages: vec![
            Message::User(UserMessage {
                content: blocks.clone(),
                timestamp: 1,
            }),
            Message::ToolResult(ToolResultMessage {
                content: blocks.clone(),
                details: None,
                ..match result("id") {
                    Message::ToolResult(r) => r,
                    _ => panic!(),
                }
            }),
        ],
        ..context()
    };
    assert_eq!(
        project_context(&source, &target, &|_, _, _| panic!(), 73),
        source
    );
}

#[test]
fn projection_never_mutates_source_context() {
    let mut source = replay_context();
    source.tools[0].parameters = json!({"type":"object","properties":{"x":{"type":"integer"}}});
    source.messages.insert(
        0,
        Message::User(UserMessage {
            content: vec![input("hello"), image("AAECAw==")],
            timestamp: 3,
        }),
    );
    source
        .messages
        .push(Message::Assistant(assistant(vec![call("trailing")])));
    let before = source.clone();
    for mode in 0..3 {
        let mut target = model();
        if mode == 0 {
            target.input.push("image".into());
        }
        if mode == 1 {
            target.identity.provider = "foreign".into();
        }
        let mut projected = project_context(&source, &target, &|id, _, _| format!("safe-{id}"), 73);
        assert_eq!(
            projected,
            project_context(&source, &target, &|id, _, _| format!("safe-{id}"), 73)
        );
        projected.tools[0].parameters["properties"]["x"]["type"] = json!("string");
        projected.system_prompt.as_mut().unwrap().clear();
        let Message::User(u) = &mut projected.messages[0] else {
            panic!()
        };
        u.content.clear();
        let Message::Assistant(a) = &mut projected.messages[1] else {
            panic!()
        };
        let AssistantContent::ToolCall(c) = a.content.last().unwrap() else {
            panic!()
        };
        let mut args = c.arguments().unwrap().clone();
        args.insert("x".into(), json!(999));
        *a.content.last_mut().unwrap() = AssistantContent::ToolCall(ToolCall::new(
            "changed".into(),
            "changed".into(),
            args,
            None,
        ));
        assert_eq!(source, before);
        let fake = Arc::new(ScriptedProvider::new(vec![steps(vec![done()])]));
        let mut models = Models::new(Arc::new(|| 73));
        models.register(target.clone(), fake.clone()).unwrap();
        block_on(models.complete(target, source.clone(), support::auth::local()));
        let mut observed = fake.calls();
        observed[0].context.messages.clear();
        assert!(!fake.calls()[0].context.messages.is_empty());
        assert_eq!(source, before);
    }
}

struct IdProvider(Arc<ScriptedProvider>);
impl Provider for IdProvider {
    fn supports(&self, operation: &str) -> bool {
        self.0.supports(operation)
    }
    fn normalize_tool_call_id(&self, id: &str, _: &Model, _: &AssistantMessage) -> String {
        format!("adapter-{id}")
    }
    fn stream(
        &self,
        model: Model,
        context: Context,
        options: ProviderOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        self.0.stream(model, context, options)
    }
}

#[test]
fn model_substitution_keeps_callers_unchanged() {
    fn caller(models: &Models, target: Model, source: &Context) -> AssistantMessage {
        block_on(models.complete(target, source.clone(), support::auth::local()))
    }
    let mut source = replay_context();
    source.messages.insert(
        0,
        Message::User(UserMessage {
            content: vec![image("AAECAw==")],
            timestamp: 1,
        }),
    );
    let before = source.clone();
    let mut first = model();
    first.input.push("image".into());
    let mut second = model();
    second.identity.provider = "other".into();
    let mut third = second.clone();
    third.identity.model = "id-rule".into();
    let scripts = || {
        vec![Script::Factory(Box::new(|call| {
            Box::pin(async move {
                assert_eq!(call.context.system_prompt.as_deref(), Some("prompt"));
                assert_eq!(call.context.tools, vec![declaration()]);
                assert_eq!(call.context.messages.len(), 3);
                Ok(vec![ScriptStep::Update(done())])
            })
        }))]
    };
    let fakes: Vec<_> = (0..3)
        .map(|_| Arc::new(ScriptedProvider::new(scripts())))
        .collect();
    let mut models = Models::new(Arc::new(|| 73));
    models.register(first.clone(), fakes[0].clone()).unwrap();
    models.register(second.clone(), fakes[1].clone()).unwrap();
    models
        .register(third.clone(), Arc::new(IdProvider(fakes[2].clone())))
        .unwrap();
    for target in [first, second, third] {
        let result = caller(&models, target.clone(), &source);
        assert_eq!(result.stop_reason, Some(StopReason::Stop));
        assert_eq!(result.model, target.identity.model);
        assert_eq!(result.timestamp, 73);
    }
    let contexts: Vec<_> = fakes
        .iter()
        .map(|fake| {
            assert_eq!(fake.calls().len(), 1);
            fake.calls()[0].context.clone()
        })
        .collect();
    let Message::User(u) = &contexts[0].messages[0] else {
        panic!()
    };
    assert_eq!(u.content, vec![image("AAECAw==")]);
    let Message::User(u) = &contexts[1].messages[0] else {
        panic!()
    };
    assert_eq!(
        u.content,
        vec![input("(image omitted: model does not support images)")]
    );
    let Message::Assistant(a) = &contexts[2].messages[1] else {
        panic!()
    };
    let AssistantContent::ToolCall(c) = a.content.last().unwrap() else {
        panic!()
    };
    assert_eq!(c.id, "adapter-original");
    let Message::ToolResult(r) = &contexts[2].messages[2] else {
        panic!()
    };
    assert_eq!(r.tool_call_id, "adapter-original");
    assert_eq!(source, before);
}
