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
        parameters: json!({"type":"object"}).into(),
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
        arguments: serde_json::Value::Object(arguments),
        thought_signature,
    }))
}

fn user_blocks(blocks: Vec<InputContent>) -> Message {
    Message::User(UserMessage {
        content: UserContent::Blocks(blocks),
        timestamp: 17.0,
    })
}
fn tool_blocks(blocks: Vec<InputContent>) -> Message {
    let Message::ToolResult(mut r) = result("unpaired") else {
        panic!()
    };
    r.content = blocks;
    Message::ToolResult(r)
}
fn blocks(message: &Message) -> &[InputContent] {
    match message {
        Message::User(UserMessage {
            content: UserContent::Blocks(b),
            ..
        }) => b,
        Message::ToolResult(r) => &r.content,
        _ => panic!(),
    }
}
fn results(messages: &[Message]) -> Vec<&ToolResultMessage> {
    messages
        .iter()
        .filter_map(|m| {
            if let Message::ToolResult(r) = m {
                Some(r)
            } else {
                None
            }
        })
        .collect()
}
fn calls(message: &Message) -> Vec<Arc<std::sync::RwLock<ToolCall>>> {
    let Message::Assistant(a) = message else {
        panic!()
    };
    a.content
        .iter()
        .filter_map(|b| {
            if let AssistantContent::ToolCall(c) = b {
                Some(c.clone())
            } else {
                None
            }
        })
        .collect()
}
fn foreign_call(ids: &[&str]) -> Message {
    let mut a = assistant(ids.iter().map(|id| call(id)).collect());
    a.provider = "foreign".into();
    Message::Assistant(a)
}
fn now() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64
}

#[test]
fn image_capability_preserves_strings_blocks_and_details() {
    let original = vec![
        Message::User(UserMessage {
            content: UserContent::Text("literal".into()),
            timestamp: 3.0,
        }),
        user_blocks(vec![]),
        tool_blocks(vec![]),
        user_blocks(vec![
            image("AAECA/+/=="),
            InputContent::Image(ImageContent {
                data: "exact:opaque-base64".into(),
                mime_type: "image/custom+binary".into(),
            }),
            InputContent::Text(TextContent {
                text: "text".into(),
                text_signature: Some("opaque".into()),
            }),
        ]),
        tool_blocks(vec![image("binary"), input("text")]),
        Message::Assistant(assistant(vec![AssistantContent::Text(TextContent {
            text: "assistant".into(),
            text_signature: Some("signed".into()),
        })])),
    ];
    assert!(transform_messages(&[], &model(), None).is_empty());
    let mut target = model();
    target.input.push("image".into());
    assert_eq!(transform_messages(&original, &target, None), original);
    for capability in ["IMAGE", "audio", "other"] {
        target.input = vec![capability.into()];
        let output = transform_messages(&original, &target, None);
        assert_eq!(&output[..3], &original[..3]);
        assert_eq!(output[5], original[5]);
        assert_eq!(
            blocks(&output[3])[0],
            input("(image omitted: model does not support images)")
        );
        assert_eq!(
            blocks(&output[4])[0],
            input("(tool image omitted: model does not support images)")
        );
        assert_eq!(
            results(&output)[1].details,
            Some(json!({"private":"sentinel"}))
        );
    }
}

#[test]
fn image_runs_use_distinct_exact_placeholders() {
    for (make, literal) in [
        (
            user_blocks as fn(Vec<InputContent>) -> Message,
            "(image omitted: model does not support images)",
        ),
        (
            tool_blocks,
            "(tool image omitted: model does not support images)",
        ),
    ] {
        for (original, expected) in [
            (vec![], vec![]),
            (vec![image("1"), image("2")], vec![input(literal)]),
            (
                vec![
                    image("1"),
                    image("2"),
                    input("middle"),
                    image("3"),
                    input(""),
                    image("4"),
                    image("5"),
                ],
                vec![
                    input(literal),
                    input("middle"),
                    input(literal),
                    input(""),
                    input(literal),
                ],
            ),
        ] {
            let source = vec![make(original.clone())];
            assert_eq!(
                blocks(&transform_messages(&source, &model(), None)[0]),
                expected
            );
            assert_eq!(blocks(&source[0]), original);
        }
    }
}

#[test]
fn literal_placeholder_blocks_keep_their_metadata() {
    for (make, literal, other) in [
        (
            user_blocks as fn(Vec<InputContent>) -> Message,
            "(image omitted: model does not support images)",
            "(tool image omitted: model does not support images)",
        ),
        (
            tool_blocks,
            "(tool image omitted: model does not support images)",
            "(image omitted: model does not support images)",
        ),
    ] {
        let signed = |signature: &str| {
            InputContent::Text(TextContent {
                text: literal.into(),
                text_signature: Some(signature.into()),
            })
        };
        let original = vec![
            signed("one"),
            signed("two"),
            image("suppressed"),
            input(other),
            image("not suppressed"),
            input(""),
            image("generated"),
            signed("three"),
        ];
        let expected = vec![
            signed("one"),
            signed("two"),
            input(other),
            input(literal),
            input(""),
            input(literal),
            signed("three"),
        ];
        let source = vec![make(original.clone())];
        let output = transform_messages(&source, &model(), None);
        assert_eq!(blocks(&output[0]), expected);
        assert_eq!(transform_messages(&output, &model(), None), output);
        assert_eq!(blocks(&source[0]), original);
    }
}

#[test]
fn same_model_requires_all_three_identity_fields() {
    let mut a = assistant(vec![AssistantContent::Text(TextContent {
        text: "answer".into(),
        text_signature: Some("signed".into()),
    })]);
    a.error_message = Some("retained".into());
    a.diagnostics = Some(vec![AssistantMessageDiagnostic {
        r#type: "custom".into(),
        timestamp: 5.0,
        error: None,
        details: Some(json!({"retained":true}).as_object().unwrap().clone()),
    }]);
    for mismatch in 0..4 {
        let mut target = model();
        match mismatch {
            1 => target.provider = "foreign".into(),
            2 => target.api = "foreign".into(),
            3 => target.id = "foreign".into(),
            _ => {}
        }
        let output = transform_messages(&[Message::Assistant(a.clone())], &target, None);
        let mut expected = a.clone();
        if mismatch != 0 {
            expected.content = vec![AssistantContent::Text(TextContent {
                text: "answer".into(),
                text_signature: None,
            })];
        }
        assert_eq!(output, vec![Message::Assistant(expected)]);
    }
}

#[test]
fn thinking_replay_obeys_redaction_and_signature_truthiness() {
    for same in [false, true] {
        for redacted in [None, Some(false), Some(true)] {
            for text in ["", " \n", " readable "] {
                for signature in [None, Some(""), Some("signed"), Some(" ")] {
                    let block = AssistantContent::Thinking(ThinkingContent {
                        thinking: text.into(),
                        thinking_signature: signature.map(str::to_owned),
                        redacted,
                    });
                    let mut target = model();
                    if !same {
                        target.provider = "foreign".into();
                    }
                    let output = transform_messages(
                        &[Message::Assistant(assistant(vec![block.clone()]))],
                        &target,
                        None,
                    );
                    let Message::Assistant(a) = &output[0] else {
                        panic!()
                    };
                    let expected = if redacted == Some(true) {
                        if same { vec![block] } else { vec![] }
                    } else if same && signature.is_some_and(|s| !s.is_empty()) {
                        vec![block]
                    } else if text.is_empty() || text == " \n" {
                        vec![]
                    } else if same {
                        vec![block]
                    } else {
                        vec![AssistantContent::Text(TextContent {
                            text: text.into(),
                            text_signature: None,
                        })]
                    };
                    assert_eq!(
                        a.content, expected,
                        "same={same} redacted={redacted:?} text={text:?} signature={signature:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn thinking_blank_detection_uses_ecmascript_whitespace() {
    let whitespace = "\u{0009}\u{000a}\u{000b}\u{000c}\u{000d}\u{0020}\u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}\u{feff}";
    for (characters, blank) in [(whitespace, true), ("\u{0085}\u{180e}\u{200b}", false)] {
        for c in characters.chars() {
            for same in [false, true] {
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
                        let mut target = model();
                        if !same {
                            target.id = "foreign".into();
                        }
                        let output = transform_messages(
                            &[Message::Assistant(assistant(vec![block.clone()]))],
                            &target,
                            None,
                        );
                        let Message::Assistant(a) = &output[0] else {
                            panic!()
                        };
                        let expected = if blank && !readable && (!same || signature.is_none()) {
                            vec![]
                        } else if same {
                            vec![block]
                        } else {
                            vec![AssistantContent::Text(TextContent {
                                text,
                                text_signature: None,
                            })]
                        };
                        assert_eq!(a.content, expected, "U+{:04X}", c as u32);
                    }
                }
            }
        }
    }
}

#[test]
fn foreign_text_and_call_signatures_have_distinct_rules() {
    for same in [false, true] {
        for signature in [None, Some(""), Some("signed"), Some(" ")] {
            for rewrite in [false, true] {
                let call = AssistantContent::ToolCall(tool_call(
                    "a".into(),
                    "lookup".into(),
                    json!({"x":1}).as_object().unwrap().clone(),
                    signature.map(str::to_owned),
                ));
                let text = AssistantContent::Text(TextContent {
                    text: String::new(),
                    text_signature: signature.map(str::to_owned),
                });
                let mut target = model();
                if !same {
                    target.api = "foreign".into();
                }
                let output = transform_messages(
                    &[Message::Assistant(assistant(vec![text.clone(), call]))],
                    &target,
                    Some(&mut |id, _, _| if rewrite { "new".into() } else { id.into() }),
                );
                let Message::Assistant(a) = &output[0] else {
                    panic!()
                };
                assert_eq!(
                    a.content[0],
                    if same {
                        text
                    } else {
                        AssistantContent::Text(TextContent {
                            text: String::new(),
                            text_signature: None,
                        })
                    }
                );
                let c = calls(&output[0]);
                let c = c[0].read().unwrap();
                assert_eq!(
                    c.thought_signature.as_deref(),
                    if same || signature == Some("") {
                        signature
                    } else {
                        None
                    }
                );
                assert_eq!(c.id, if !same && rewrite { "new" } else { "a" });
            }
        }
    }
}

#[test]
fn unchanged_calls_share_handles_changed_calls_do_not() {
    for same in [false, true] {
        for signed in [false, true] {
            for rewrite in [false, true] {
                let handle = tool_call(
                    "a".into(),
                    "lookup".into(),
                    json!({"x":1}).as_object().unwrap().clone(),
                    signed.then(|| "signed".into()),
                );
                let source = vec![Message::Assistant(assistant(vec![
                    AssistantContent::ToolCall(handle.clone()),
                ]))];
                let snapshot = serde_json::to_value(&source).unwrap();
                let mut target = model();
                if !same {
                    target.provider = "foreign".into();
                }
                let output = transform_messages(
                    &source,
                    &target,
                    Some(&mut |id, _, origin| {
                        let AssistantContent::ToolCall(c) = &origin.content[0] else {
                            panic!()
                        };
                        assert!(c.try_write().is_ok());
                        if rewrite { "new".into() } else { id.into() }
                    }),
                );
                assert_eq!(serde_json::to_value(&source).unwrap(), snapshot);
                let projected = calls(&output[0]);
                let shared = same || (!signed && !rewrite);
                assert_eq!(Arc::ptr_eq(&handle, &projected[0]), shared);
                handle.write().unwrap().name = "later".into();
                assert_eq!(
                    projected[0].read().unwrap().name,
                    if shared { "later" } else { "lookup" }
                );
            }
        }
    }
}

#[test]
fn normalizer_is_optional_foreign_only_and_source_ordered() {
    let mut failed = match foreign_call(&["failed"]) {
        Message::Assistant(a) => a,
        _ => panic!(),
    };
    failed.stop_reason = StopReason::Error;
    let source = vec![
        foreign_call(&["a|b", "second"]),
        Message::Assistant(assistant(vec![call("same")])),
        Message::Assistant(failed),
    ];
    let absent = transform_messages(&source, &model(), None);
    assert_eq!(calls(&absent[0])[0].read().unwrap().id, "a|b");
    let mut seen = vec![];
    let output = transform_messages(
        &source,
        &model(),
        Some(&mut |id, target, origin| {
            assert_eq!(target, &model());
            assert_eq!(origin.provider, "foreign");
            assert_eq!(
                origin
                    .content
                    .iter()
                    .filter(|b| matches!(b, AssistantContent::ToolCall(_)))
                    .count(),
                if id == "failed" { 1 } else { 2 }
            );
            seen.push((
                id.to_owned(),
                origin.stop_reason.clone(),
                origin.content.clone(),
            ));
            id.into()
        }),
    );
    assert_eq!(
        seen.iter()
            .map(|(id, _, _)| id.as_str())
            .collect::<Vec<_>>(),
        vec!["a|b", "second", "failed"]
    );
    assert_eq!(seen[2].1, StopReason::Error);
    assert_eq!(
        seen[0].2,
        match &source[0] {
            Message::Assistant(a) => a.content.clone(),
            _ => panic!(),
        }
    );
    assert_eq!(calls(&output[0])[0].read().unwrap().id, "a|b");
    transform_messages(
        &[Message::Assistant(assistant(vec![call("same")]))],
        &model(),
        Some(&mut |_, _, _| panic!("same model callback")),
    );
}

#[test]
fn changed_id_mappings_survive_identity_and_same_model_reuse() {
    let source = vec![
        result("a"),
        foreign_call(&["a"]),
        result("a"),
        Message::Assistant(assistant(vec![call("a")])),
        result("a"),
        foreign_call(&["a"]),
        result("a"),
        foreign_call(&["a"]),
        user_blocks(vec![input("interrupt")]),
        Message::Assistant(assistant(vec![])),
        result("a"),
    ];
    let mut invocation = 0;
    let output = transform_messages(
        &source,
        &model(),
        Some(&mut |_, _, _| {
            invocation += 1;
            match invocation {
                1 => "x".into(),
                2 => "a".into(),
                _ => "y".into(),
            }
        }),
    );
    assert_eq!(
        results(&output)
            .iter()
            .map(|r| (r.tool_call_id.as_str(), r.is_error))
            .collect::<Vec<_>>(),
        vec![
            ("a", false),
            ("x", false),
            ("x", false),
            ("a", true),
            ("x", false),
            ("a", true),
            ("y", true),
            ("y", false)
        ]
    );
    let a_ids = output
        .iter()
        .filter(|m| matches!(m, Message::Assistant(_)))
        .map(|m| {
            calls(m)
                .into_iter()
                .map(|c| c.read().unwrap().id.clone())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        a_ids,
        vec![vec!["x"], vec!["a"], vec!["a"], vec!["y"], vec![]]
    );
    assert_eq!(
        results(&output).last().unwrap().details,
        Some(json!({"private":"sentinel"}))
    );
}

#[test]
fn empty_normalized_ids_do_not_rewrite_real_results() {
    let source = vec![
        foreign_call(&["a"]),
        result("a"),
        foreign_call(&["a"]),
        result("a"),
    ];
    let mut invocation = 0;
    let output = transform_messages(
        &source,
        &model(),
        Some(&mut |_, _, _| {
            invocation += 1;
            if invocation == 1 {
                "x".into()
            } else {
                String::new()
            }
        }),
    );
    assert_eq!(
        results(&output)
            .iter()
            .map(|r| (r.tool_call_id.as_str(), r.is_error))
            .collect::<Vec<_>>(),
        vec![("x", false), ("a", false), ("", true)]
    );
    assert_eq!(calls(&output[2])[0].read().unwrap().id, "");
}

#[test]
fn colliding_ids_match_as_a_set_without_deduplicating_calls() {
    for ids in [vec!["a", "b"], vec!["a", "a"]] {
        for answered in [0, 1, 2] {
            let mut source = vec![foreign_call(&ids)];
            for _ in 0..answered {
                source.push(result("a"));
            }
            let output = transform_messages(&source, &model(), Some(&mut |_, _, _| "x".into()));
            let r = results(&output);
            assert_eq!(r.len(), if answered == 0 { 2 } else { answered });
            assert!(r.iter().all(|r| r.tool_call_id == "x"));
            assert!(r.iter().all(|r| r.is_error == (answered == 0)));
            if answered == 2 {
                assert_eq!(r[0], r[1]);
            }
            assert_eq!(calls(&output[0]).len(), 2);
        }
    }
}

#[test]
fn failed_assistants_normalize_before_second_pass_omission() {
    for reason in [StopReason::Error, StopReason::Aborted] {
        let mut failed = match foreign_call(&["failed"]) {
            Message::Assistant(a) => a,
            _ => panic!(),
        };
        failed.stop_reason = reason;
        failed.error_message = Some(Failure::Transport.to_string());
        let source = vec![
            Message::Assistant(assistant(vec![call("good")])),
            Message::Assistant(failed),
            result("failed"),
        ];
        let mut seen = vec![];
        let output = transform_messages(
            &source,
            &model(),
            Some(&mut |id, _, _| {
                seen.push(id.to_owned());
                format!("new-{id}")
            }),
        );
        assert_eq!(seen, vec!["failed"]);
        assert_eq!(output.len(), 3);
        assert_eq!(output[0], source[0]);
        assert_eq!(
            results(&output)
                .iter()
                .map(|r| (r.tool_call_id.as_str(), r.is_error))
                .collect::<Vec<_>>(),
            vec![("good", true), ("new-failed", false)]
        );
        assert_eq!(
            results(&output)[1].details,
            Some(json!({"private":"sentinel"}))
        );
    }
    for reason in [StopReason::Stop, StopReason::Length, StopReason::ToolUse] {
        let mut a = assistant(vec![]);
        a.stop_reason = reason;
        let source = vec![Message::Assistant(a)];
        assert_eq!(transform_messages(&source, &model(), None), source);
    }
}

#[test]
fn normalizer_callback_observes_unlocked_original_calls() {
    for signature in [None, Some(""), Some("signed")] {
        for returned in ["normalized", "mutated"] {
            let handle = tool_call(
                "original".into(),
                "lookup".into(),
                json!({"x":1}).as_object().unwrap().clone(),
                signature.map(str::to_owned),
            );
            let mut a = assistant(vec![AssistantContent::ToolCall(handle.clone())]);
            a.provider = "foreign".into();
            let source = vec![Message::Assistant(a), result("mutated")];
            let output = transform_messages(
                &source,
                &model(),
                Some(&mut |id, _, origin| {
                    assert_eq!(id, "original");
                    let AssistantContent::ToolCall(c) = &origin.content[0] else {
                        panic!()
                    };
                    let mut c = c.try_write().expect("normalizer must run without a guard");
                    assert_eq!(c.thought_signature.as_deref(), signature);
                    c.id = "mutated".into();
                    c.arguments
                        .as_object_mut()
                        .unwrap()
                        .insert("x".into(), json!(2));
                    returned.into()
                }),
            );
            let projected = calls(&output[0]);
            let projected = projected[0].read().unwrap();
            let stripped = signature == Some("signed");
            assert_eq!(
                projected.id,
                if returned == "normalized" {
                    "normalized"
                } else if stripped {
                    "original"
                } else {
                    "mutated"
                }
            );
            assert_eq!(
                projected.arguments["x"],
                json!(if stripped { 1 } else { 2 })
            );
            assert_eq!(
                projected.thought_signature.as_deref(),
                if stripped { None } else { signature }
            );
            let r = results(&output);
            assert_eq!(r[0].tool_call_id, returned);
            assert_eq!(
                r.len(),
                if stripped && returned == "mutated" {
                    2
                } else {
                    1
                }
            );
            if r.len() == 2 {
                assert_eq!(r[1].tool_call_id, "original");
                assert!(r[1].is_error);
            }
        }
    }
}

#[test]
fn missing_calls_are_repaired_at_each_boundary() {
    let mut failed = assistant(vec![call("ignored")]);
    failed.stop_reason = StopReason::Error;
    for boundary in [
        None,
        Some(Message::Assistant(assistant(vec![]))),
        Some(Message::Assistant(failed)),
        Some(Message::User(UserMessage {
            content: UserContent::Text("interrupt".into()),
            timestamp: 22.0,
        })),
        Some(user_blocks(vec![input("interrupt")])),
    ] {
        for answered in 0..=2 {
            let mut source = vec![
                result("a"),
                result("unrelated"),
                Message::Assistant(assistant(vec![call("a"), call("b")])),
            ];
            for id in ["a", "b"].into_iter().take(answered) {
                source.push(result(id));
                source.push(result(id));
            }
            source.push(result("unrelated"));
            if let Some(b) = &boundary {
                source.push(b.clone());
            }
            let before = serde_json::to_value(&source).unwrap();
            let output = transform_messages(&source, &model(), None);
            assert_eq!(serde_json::to_value(&source).unwrap(), before);
            let missing = results(&output)
                .into_iter()
                .filter(|r| r.is_error)
                .collect::<Vec<_>>();
            assert_eq!(
                missing
                    .iter()
                    .map(|r| r.tool_call_id.as_str())
                    .collect::<Vec<_>>(),
                ["a", "b"][answered..]
            );
            for r in &missing {
                assert_eq!(r.tool_name, "lookup");
                assert_eq!(r.content, vec![input("No result provided")]);
                assert_eq!(r.details, None);
            }
            let real_output = output
                .iter()
                .filter(|m| !matches!(m,Message::ToolResult(r) if r.is_error))
                .filter(|m| !matches!(m,Message::Assistant(a) if a.stop_reason==StopReason::Error))
                .cloned()
                .collect::<Vec<_>>();
            let real_source = source
                .iter()
                .filter(|m| !matches!(m,Message::Assistant(a) if a.stop_reason==StopReason::Error))
                .cloned()
                .collect::<Vec<_>>();
            assert_eq!(real_output, real_source);
            if let Some(b) = &boundary
                && !matches!(b,Message::Assistant(a) if a.stop_reason==StopReason::Error)
            {
                assert_eq!(output.last(), Some(b));
            }
            assert_eq!(transform_messages(&output, &model(), None), output);
        }
    }
    for source in [
        vec![],
        vec![result("unrelated")],
        vec![
            Message::Assistant(assistant(vec![])),
            result("a"),
            user_blocks(vec![]),
        ],
    ] {
        assert_eq!(transform_messages(&source, &model(), None), source);
    }
    let source = vec![foreign_call(&["a", "b"]), user_blocks(vec![])];
    let output = transform_messages(&source, &model(), Some(&mut |id, _, _| format!("new-{id}")));
    assert_eq!(
        results(&output)
            .iter()
            .map(|r| r.tool_call_id.as_str())
            .collect::<Vec<_>>(),
        vec!["new-a", "new-b"]
    );
    assert_eq!(output.last(), source.last());
}

#[test]
fn transformation_preserves_input_values_and_result_details() {
    let mut source = replay_context();
    source
        .messages
        .insert(0, user_blocks(vec![input("hello"), image("AAECAw==")]));
    for details in [
        None,
        Some(json!(null)),
        Some(json!({"retained":{"x":[1,2]}})),
    ] {
        let Message::ToolResult(mut r) = result("original") else {
            panic!()
        };
        r.details = details;
        r.is_error = true;
        source.messages.push(Message::ToolResult(r));
    }
    let before = serde_json::to_value(&source).unwrap();
    let mut target = model();
    target.provider = "foreign".into();
    let output = transform_messages(
        &source.messages,
        &target,
        Some(&mut |id, _, _| format!("safe-{id}")),
    );
    assert_eq!(serde_json::to_value(&source).unwrap(), before);
    for (original, actual) in results(&source.messages).iter().zip(results(&output)) {
        let mut expected = (*original).clone();
        expected.tool_call_id = "safe-original".into();
        assert_eq!(actual, &expected);
    }
    let Message::Assistant(a) = &output[1] else {
        panic!()
    };
    let Message::Assistant(original) = &source.messages[1] else {
        panic!()
    };
    let mut metadata = original.clone();
    metadata.content = a.content.clone();
    assert_eq!(a, &metadata);
    assert_eq!(
        calls(&output[1])[0].read().unwrap().arguments,
        json!({"x":1})
    );
    assert_eq!(source.system_prompt.as_deref(), Some("prompt"));
    assert_eq!(
        serde_json::to_value(&source.tools).unwrap(),
        serde_json::to_value(Some(vec![declaration()])).unwrap()
    );
    let prompt_and_tools = context();
    assert_eq!(
        prompt_and_tools.system_prompt.as_deref(),
        Some("synthetic secret")
    );
}

#[test]
fn synthetic_timestamps_are_sampled_inside_the_call() {
    let source = vec![Message::Assistant(assistant(vec![
        call("a"),
        call("b"),
        call("c"),
    ]))];
    let before = now();
    let output = transform_messages(&source, &model(), None);
    let after = now();
    let r = results(&output);
    assert_eq!(r.len(), 3);
    for r in r {
        assert!(r.timestamp >= before && r.timestamp <= after);
        assert_eq!(r.timestamp.fract(), 0.0);
    }
    let mut answered = source;
    answered.extend([result("a"), result("b"), result("c")]);
    assert_eq!(transform_messages(&answered, &model(), None), answered);
}
