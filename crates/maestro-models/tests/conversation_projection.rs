mod support;
use maestro_models::*;
use serde_json::json;
use std::sync::Arc;

fn input(text: &str) -> InputContent {
    InputContent::Text(TextContent {
        text: text.into(),
        text_signature: None,
    })
}
fn image(data: &str) -> InputContent {
    InputContent::Image(ImageContent {
        data: data.into(),
        mime_type: "image/png".into(),
    })
}
fn declaration() -> Tool {
    Tool {
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
        timestamp: 19.0,
    })
}

fn assistant(blocks: Vec<AssistantContent>) -> AssistantMessage {
    AssistantMessage {
        provider: model().provider,
        api: model().api,
        model: model().id,
        timestamp: 11.0,
        content: blocks,
        usage: usage(),
        stop_reason: StopReason::ToolUse,
        error_message: None,
        diagnostics: None,
        response_model: Some("different-actual".into()),
        response_id: Some("response".into()),
    }
}
fn call(id: &str) -> AssistantContent {
    AssistantContent::ToolCall(tool_call(
        id.into(),
        "lookup".into(),
        json!({"x":1}).as_object().unwrap().clone(),
        Some("call-signature".into()),
    ))
}
fn replay_context() -> Context {
    Context {
        system_prompt: Some("prompt".into()),
        tools: Some(vec![declaration()]),
        messages: vec![
            Message::Assistant(assistant(vec![
                AssistantContent::Text(TextContent {
                    text: "answer".into(),
                    text_signature: Some("text-signature".into()),
                }),
                AssistantContent::Thinking(ThinkingContent {
                    thinking: "reason".into(),
                    thinking_signature: Some("thinking-signature".into()),
                    redacted: None,
                }),
                AssistantContent::Thinking(ThinkingContent {
                    thinking: "".into(),
                    thinking_signature: Some("empty-signature".into()),
                    redacted: None,
                }),
                AssistantContent::Thinking(ThinkingContent {
                    thinking: "  \n".into(),
                    thinking_signature: None,
                    redacted: None,
                }),
                AssistantContent::Thinking(ThinkingContent {
                    thinking: String::new(),
                    thinking_signature: Some("opaque-redaction".into()),
                    redacted: Some(true),
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
            0 => target.provider = "foreign".into(),
            1 => target.api = "foreign".into(),
            _ => target.id = "foreign".into(),
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
                    text_signature: None
                }),
                AssistantContent::Text(TextContent {
                    text: "reason".into(),
                    text_signature: None
                }),
                AssistantContent::ToolCall(tool_call(
                    "original".into(),
                    "lookup".into(),
                    json!({"x":1}).as_object().unwrap().clone(),
                    None
                ))
            ]
        );
        assert_eq!(a.timestamp, 11.0);
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
    assert_eq!(c.read().unwrap().id, "new-a");
    assert_eq!(c.read().unwrap().name, "lookup");
    assert_eq!(
        &c.read().unwrap().arguments,
        json!({"x":1}).as_object().unwrap()
    );
    let Message::ToolResult(synthetic) = &projected.messages[3] else {
        panic!()
    };
    assert_eq!(synthetic.content, vec![input("No result provided")]);
    assert_eq!(synthetic.timestamp, 73.0);
    assert!(synthetic.is_error);
    assert_eq!(source.messages[1], Message::Assistant(foreign));
}

#[test]
fn missing_results_are_completed_at_each_turn_boundary() {
    let mut failed = assistant(vec![call("ignored")]);
    failed.stop_reason = StopReason::Error;
    failed.error_message = Some(Failure::Transport.to_string());
    let boundaries = [
        None,
        Some(Message::User(UserMessage {
            content: UserContent::Blocks(vec![input("interrupt")]),
            timestamp: 22.0,
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
                assert_eq!(r.timestamp, 99.0);
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
    let partial_call = AssistantContent::ToolCall(Arc::new(std::sync::RwLock::new(ToolCall {
        id: "partial".into(),
        name: "lookup".into(),
        arguments: Default::default(),
        thought_signature: None,
    })));
    let mut source = context();
    source.messages.extend([
        Message::Assistant(assistant(vec![call("good")])),
        result("good"),
    ]);
    let expected_source = source.clone();
    for reason in [StopReason::Error, StopReason::Aborted] {
        let mut attempt = assistant(vec![
            AssistantContent::Text(TextContent {
                text: "partial".into(),
                text_signature: None,
            }),
            partial_call.clone(),
        ]);
        attempt.stop_reason = reason;
        attempt.error_message = Some(Failure::Transport.to_string());
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
                content: UserContent::Blocks(blocks.clone()),
                timestamp: 1.0,
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
                Message::User(u) => match &u.content {
                    UserContent::Blocks(content) => content,
                    _ => panic!(),
                },
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
                Message::User(u) => match &u.content {
                    UserContent::Blocks(content) => content,
                    _ => panic!(),
                },
                _ => panic!(),
            },
            &blocks
        );
    }
    let source = Context {
        messages: vec![Message::User(UserMessage {
            content: UserContent::Blocks(vec![
                input("(image omitted: model does not support images)"),
                image("AA=="),
            ]),
            timestamp: 1.0,
        })],
        ..context()
    };
    let projected = project_context(&source, &model(), &|id, _, _| id.into(), 73);
    let Message::User(u) = &projected.messages[0] else {
        panic!()
    };
    assert_eq!(
        u.content,
        UserContent::Blocks(vec![input(
            "(image omitted: model does not support images)"
        )])
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
                content: UserContent::Blocks(blocks.clone()),
                timestamp: 1.0,
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
    source.tools.as_mut().unwrap()[0].parameters =
        json!({"type":"object","properties":{"x":{"type":"integer"}}});
    source.messages.insert(
        0,
        Message::User(UserMessage {
            content: UserContent::Blocks(vec![input("hello"), image("AAECAw==")]),
            timestamp: 3.0,
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
            target.provider = "foreign".into();
        }
        let mut projected = project_context(&source, &target, &|id, _, _| format!("safe-{id}"), 73);
        assert_eq!(
            projected,
            project_context(&source, &target, &|id, _, _| format!("safe-{id}"), 73)
        );
        projected.tools.as_mut().unwrap()[0].parameters["properties"]["x"]["type"] =
            json!("string");
        projected.system_prompt.as_mut().unwrap().clear();
        let Message::User(u) = &mut projected.messages[0] else {
            panic!()
        };
        let UserContent::Blocks(content) = &mut u.content else {
            panic!()
        };
        content.clear();
        let Message::Assistant(a) = &mut projected.messages[1] else {
            panic!()
        };
        let AssistantContent::ToolCall(c) = a.content.last().unwrap() else {
            panic!()
        };
        let mut args = c.read().unwrap().arguments.clone();
        args.insert("x".into(), json!(999));
        *a.content.last_mut().unwrap() =
            AssistantContent::ToolCall(tool_call("changed".into(), "changed".into(), args, None));
        assert_eq!(source, before);
    }
}

fn omission_source(content: Vec<InputContent>, tool: bool) -> Context {
    let message = if tool {
        let Message::ToolResult(mut r) = result("unpaired") else {
            panic!()
        };
        r.content = content;
        Message::ToolResult(r)
    } else {
        Message::User(UserMessage {
            content: UserContent::Blocks(content),
            timestamp: 17.0,
        })
    };
    Context {
        messages: vec![message],
        ..context()
    }
}
fn omission_content(context: &Context) -> &[InputContent] {
    match &context.messages[0] {
        Message::User(u) => match &u.content {
            UserContent::Blocks(content) => content,
            _ => panic!(),
        },
        Message::ToolResult(r) => &r.content,
        _ => panic!(),
    }
}
#[test]
fn supplied_placeholder_text_is_never_removed() {
    for (tool, literal) in [
        (false, "(image omitted: model does not support images)"),
        (true, "(tool image omitted: model does not support images)"),
    ] {
        let signed = InputContent::Text(TextContent {
            text: literal.into(),
            text_signature: Some("opaque".into()),
        });
        let content = vec![
            input(literal),
            signed.clone(),
            input("ordinary"),
            image("1"),
            image("2"),
            input(literal),
            signed,
            input(""),
            image("3"),
        ];
        let source = omission_source(content.clone(), tool);
        let before = source.clone();
        let projected = project_context(&source, &model(), &|id, _, _| id.into(), 73);
        let expected = vec![
            content[0].clone(),
            content[1].clone(),
            input("ordinary"),
            input(literal),
            content[5].clone(),
            content[6].clone(),
            input(""),
            input(literal),
        ];
        assert_eq!(omission_content(&projected), expected);
        assert_eq!(
            project_context(&projected, &model(), &|id, _, _| id.into(), 73),
            projected
        );
        assert_eq!(source, before);
        let mut capable = model();
        capable.input.push("image".into());
        assert_eq!(
            omission_content(&project_context(
                &source,
                &capable,
                &|id, _, _| id.into(),
                73
            )),
            content
        );
    }
}

#[test]
fn placeholder_text_before_image_suppresses_another_placeholder() {
    for (tool, literal) in [
        (false, "(image omitted: model does not support images)"),
        (true, "(tool image omitted: model does not support images)"),
    ] {
        for metadata in [None, Some(""), Some("opaque")] {
            for count in [1, 3] {
                let supplied = InputContent::Text(TextContent {
                    text: literal.into(),
                    text_signature: metadata.map(str::to_owned),
                });
                let mut content = vec![supplied.clone()];
                content.extend((0..count).map(|_| image("AQID")));
                let source = omission_source(content, tool);
                let before = source.clone();
                let projected = project_context(&source, &model(), &|id, _, _| id.into(), 73);
                assert_eq!(omission_content(&projected), vec![supplied.clone()]);
                let mut interleaved = source.clone();
                match &mut interleaved.messages[0] {
                    Message::User(u) => {
                        let UserContent::Blocks(content) = &mut u.content else {
                            panic!()
                        };
                        content.extend([input("ordinary"), image("2"), image("3")])
                    }
                    Message::ToolResult(r) => {
                        r.content
                            .extend([input("ordinary"), image("2"), image("3")])
                    }
                    _ => panic!(),
                }
                assert_eq!(
                    omission_content(&project_context(
                        &interleaved,
                        &model(),
                        &|id, _, _| id.into(),
                        73
                    )),
                    vec![supplied, input("ordinary"), input(literal)]
                );
                assert_eq!(source, before);
            }
        }
    }
}

#[test]
fn delayed_real_result_keeps_rewritten_call_id() {
    let mut origin = assistant(vec![call("a")]);
    origin.provider = "foreign".into();
    let source = Context {
        messages: vec![
            Message::Assistant(origin),
            Message::User(UserMessage {
                content: UserContent::Blocks(vec![input("interrupt")]),
                timestamp: 22.0,
            }),
            result("a"),
        ],
        ..context()
    };
    let before = source.clone();
    let projected = project_context(&source, &model(), &|id, _, _| format!("adapter-{id}"), 73);
    assert_eq!(projected.messages.len(), 4);
    let Message::ToolResult(synthetic) = &projected.messages[1] else {
        panic!()
    };
    assert_eq!(synthetic.tool_call_id, "adapter-a");
    assert_eq!(synthetic.tool_name, "lookup");
    assert_eq!(synthetic.content, vec![input("No result provided")]);
    assert_eq!(synthetic.timestamp, 73.0);
    assert!(synthetic.is_error);
    let Message::ToolResult(real) = &projected.messages[3] else {
        panic!()
    };
    assert_eq!(real.tool_call_id, "adapter-a");
    assert_eq!(real.content, vec![input("found")]);
    assert_eq!(real.timestamp, 19.0);
    assert!(!real.is_error);
    assert_eq!(real.details, None);
    assert_eq!(source, before);
    for intervening in [false, true] {
        let mut origin = assistant(vec![call("a"), call("b")]);
        origin.provider = "foreign".into();
        let mut source = Context {
            messages: vec![
                Message::Assistant(origin),
                Message::User(UserMessage {
                    content: UserContent::Blocks(vec![input("interrupt")]),
                    timestamp: 22.0,
                }),
            ],
            ..context()
        };
        if intervening {
            source.messages.push(Message::Assistant(assistant(vec![])));
        }
        source.messages.extend([result("a"), result("b")]);
        let before = source.clone();
        let projected = project_context(&source, &model(), &|id, _, _| format!("adapter-{id}"), 73);
        let Message::Assistant(a) = &projected.messages[0] else {
            panic!()
        };
        for (index, id) in ["a", "b"].into_iter().enumerate() {
            let AssistantContent::ToolCall(c) = &a.content[index] else {
                panic!()
            };
            assert_eq!(c.read().unwrap().id, format!("adapter-{id}"));
            let Message::ToolResult(synthetic) = &projected.messages[index + 1] else {
                panic!()
            };
            assert_eq!(synthetic.tool_call_id, c.read().unwrap().id);
            assert_eq!(synthetic.tool_name, "lookup");
            assert_eq!(synthetic.content, vec![input("No result provided")]);
            assert!(synthetic.is_error);
            assert_eq!(synthetic.timestamp, 73.0);
            assert_eq!(synthetic.details, None);
            let Message::ToolResult(real) =
                &projected.messages[projected.messages.len() - 2 + index]
            else {
                panic!()
            };
            assert_eq!(real.tool_call_id, c.read().unwrap().id);
            assert_eq!(real.tool_name, "lookup");
            assert_eq!(real.content, vec![input("found")]);
            assert_eq!(real.timestamp, 19.0);
            assert!(!real.is_error);
            assert_eq!(real.details, None);
        }
        assert_eq!(projected.messages[3], source.messages[1]);
        assert_eq!(projected.messages.len(), source.messages.len() + 2);
        assert_eq!(source, before);
    }
}

#[test]
fn blank_thinking_with_empty_signature_is_dropped() {
    for foreign in [false, true] {
        for text in ["", " \n", "readable"] {
            for signature in [None, Some(""), Some("signed"), Some(" ")] {
                let block = AssistantContent::Thinking(ThinkingContent {
                    thinking: text.into(),
                    thinking_signature: signature.map(str::to_owned),
                    redacted: None,
                });
                let source = Context {
                    messages: vec![Message::Assistant(assistant(vec![block.clone()]))],
                    ..context()
                };
                let before = source.clone();
                let mut target = model();
                if foreign {
                    target.provider = "foreign".into();
                }
                let projected = project_context(&source, &target, &|id, _, _| id.into(), 73);
                let Message::Assistant(a) = &projected.messages[0] else {
                    panic!()
                };
                let expected =
                    if text.trim().is_empty() && (foreign || signature.is_none_or(str::is_empty)) {
                        vec![]
                    } else if foreign {
                        vec![AssistantContent::Text(TextContent {
                            text: text.into(),
                            text_signature: None,
                        })]
                    } else {
                        vec![block]
                    };
                assert_eq!(a.content, expected, "{foreign} {text:?} {signature:?}");
                assert_eq!(source, before);
            }
        }
    }
}

#[test]
fn whitespace_follows_the_standard_set() {
    let whitespace = "\u{0009}\u{000a}\u{000b}\u{000c}\u{000d}\u{0020}\u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
    for (characters, blank) in [(whitespace, true), ("\u{0085}\u{180e}\u{200b}", false)] {
        for c in characters.chars() {
            for foreign in [false, true] {
                for signature in [None, Some("signed")] {
                    for readable in [false, true] {
                        let text = if readable {
                            format!("{c}readable{c}")
                        } else {
                            c.to_string()
                        };
                        let block = AssistantContent::Thinking(ThinkingContent {
                            thinking: text.clone(),
                            thinking_signature: signature.map(str::to_owned),
                            redacted: None,
                        });
                        let source = Context {
                            messages: vec![Message::Assistant(assistant(vec![block.clone()]))],
                            ..context()
                        };
                        let before = source.clone();
                        let mut target = model();
                        if foreign {
                            target.id = "foreign".into();
                        }
                        let projected =
                            project_context(&source, &target, &|id, _, _| id.into(), 73);
                        let Message::Assistant(a) = &projected.messages[0] else {
                            panic!()
                        };
                        let expected = if blank && !readable && (foreign || signature.is_none()) {
                            vec![]
                        } else if foreign {
                            vec![AssistantContent::Text(TextContent {
                                text,
                                text_signature: None,
                            })]
                        } else {
                            vec![block]
                        };
                        assert_eq!(a.content, expected, "U+{:04X} foreign={foreign}", c as u32);
                        assert_eq!(source, before);
                    }
                }
            }
        }
    }
}

fn model() -> Model {
    Model {
        id: "text".into(),
        name: "Synthetic".into(),
        api: "synthetic".into(),
        provider: "test".into(),
        base_url: "synthetic:".into(),
        reasoning: true,
        thinking_level_map: None,
        input: vec!["text".into()],
        cost: TokenRates {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
        },
        context_window: 0.0,
        max_tokens: 0.0,
        headers: None,
        compat: None,
    }
}
fn context() -> Context {
    Context {
        system_prompt: Some("synthetic secret".into()),
        messages: vec![Message::User(UserMessage {
            content: UserContent::Blocks(vec![input("hello")]),
            timestamp: 1.0,
        })],
        tools: Some(vec![]),
    }
}
fn usage() -> Usage {
    Usage {
        input: 11.0,
        output: 7.0,
        cache_read: 3.0,
        cache_write: 2.0,
        total_tokens: 23.0,
        cost: UsageCost {
            input: 0.0,
            output: 0.0,
            cache_read: 0.0,
            cache_write: 0.0,
            total: 0.0,
        },
    }
}
fn tool_call(
    id: String,
    name: String,
    arguments: serde_json::Map<String, serde_json::Value>,
    thought_signature: Option<String>,
) -> Arc<std::sync::RwLock<ToolCall>> {
    Arc::new(std::sync::RwLock::new(ToolCall {
        id,
        name,
        arguments,
        thought_signature,
    }))
}
