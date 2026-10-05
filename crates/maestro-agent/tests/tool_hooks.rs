mod support;
use maestro_agent::*;
use maestro_models::*;
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::*;

#[tokio::test]
async fn preflight_failures_skip_execution_and_after_hooks() {
    for (case, expected) in [
        ("prepare", "prepare failed"),
        ("schema", "invalid or unresolvable tool schema"),
        ("value", "invalid tool arguments"),
        ("before", "before failed"),
        ("absent", "Tool execution was blocked"),
        ("empty", "Tool execution was blocked"),
        ("spaces", " \u{feff}\u{0085}"),
    ] {
        let mut t = tool("work", Arc::new(|_| panic!("preflight failure executed")));
        if case == "prepare" {
            t.prepare = Some(Arc::new(|_, _| {
                Box::pin(async { Err("prepare failed".into()) })
            }));
        }
        if case == "schema" {
            t.declaration.parameters = json!({"type":"invalid"});
        }
        if case == "value" {
            t.declaration.parameters = json!({"type":"object","required":["missing"]});
        }
        let hooks = if ["before", "absent", "empty", "spaces"].contains(&case) {
            vec![
                Arc::new(move |_, _| {
                    Box::pin(async move {
                        if case == "before" {
                            Err("before failed".into())
                        } else {
                            Ok(BeforeToolCallResult {
                                block: true,
                                reason: match case {
                                    "empty" => Some("".into()),
                                    "spaces" => Some(" \u{feff}\u{0085}".into()),
                                    _ => None,
                                },
                                args: Some(Default::default()),
                            })
                        }
                    })
                        as std::pin::Pin<
                            Box<
                                dyn std::future::Future<
                                        Output = Result<BeforeToolCallResult, String>,
                                    > + Send,
                            >,
                        >
                }) as BeforeToolCall,
                Arc::new(|_, _| panic!("later before hook")),
            ]
        } else {
            vec![]
        };
        let (agent, _) = setup(
            vec![
                Script::Steps(call_steps(
                    &[("id", "work", json!({}))],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![t],
                before_tool_call: hooks,
                after_tool_call: vec![Arc::new(|_, _| panic!("after preflight"))],
                ..Default::default()
            },
        );
        let events = capture(&agent);
        let r = results(&agent.prompt(user("go"), options()).unwrap().await.unwrap());
        assert!(r[0].is_error, "{case}");
        assert_eq!(r[0].content, output(expected).content, "{case}");
        assert_eq!(r[0].details, Some(json!({})));
        assert!(events.lock().unwrap().iter().any(|e| matches!(e, AgentEvent::ToolExecutionEnd { result, .. } if result.terminate.is_none())));
    }
}

#[tokio::test]
async fn before_hooks_chain_replacements_without_revalidation() {
    for replacement in [json!({"n":7}), json!({})] {
        let order = Arc::new(Mutex::new(vec![]));
        let a = order.clone();
        let b = order.clone();
        let c = order.clone();
        let expected = replacement.clone();
        let execute_expected = replacement.clone();
        let mut t = tool(
            "work",
            Arc::new(move |inv| {
                c.lock().unwrap().push("execute");
                assert_eq!(json!(inv.args), execute_expected);
                Box::pin(async { Ok(output("done")) })
            }),
        );
        t.declaration.parameters =
            json!({"type":"object","properties":{"n":{"type":"string"}},"required":["n"]});
        let before: BeforeToolCall = Arc::new(move |ctx, _| {
            a.lock().unwrap().push("first");
            assert_eq!(ctx.args["n"], "validated");
            let args = replacement.as_object().unwrap().clone();
            Box::pin(async move {
                Ok(BeforeToolCallResult {
                    args: Some(args),
                    ..Default::default()
                })
            })
        });
        let next: BeforeToolCall = Arc::new(move |ctx, _| {
            b.lock().unwrap().push("second");
            assert_eq!(json!(ctx.args), expected);
            Box::pin(async { Ok(BeforeToolCallResult::default()) })
        });
        let (agent, _) = setup(
            vec![
                Script::Steps(call_steps(
                    &[("id", "work", json!({"n":"validated"}))],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![t],
                before_tool_call: vec![before, next],
                ..Default::default()
            },
        );
        assert!(!results(&agent.prompt(user("go"), options()).unwrap().await.unwrap())[0].is_error);
        assert_eq!(*order.lock().unwrap(), vec!["first", "second", "execute"]);
    }
}

#[tokio::test]
async fn after_hooks_chain_whole_field_replacements() {
    let initial = ToolResult {
        content: output("initial").content,
        details: json!({"nested":{"old":1}}),
        terminate: Some(true),
    };
    let t = tool(
        "work",
        Arc::new(move |_| {
            let r = initial.clone();
            Box::pin(async move { Ok(r) })
        }),
    );
    let first: AfterToolCall = Arc::new(|ctx, _| {
        assert_eq!(ctx.result.details, json!({"nested":{"old":1}}));
        Box::pin(async {
            Ok(AfterToolCallResult {
                content: Some(vec![]),
                details: Some(json!({"nested":{"new":2}})),
                is_error: Some(true),
                ..Default::default()
            })
        })
    });
    let second: AfterToolCall = Arc::new(|ctx, _| {
        assert!(ctx.result.content.is_empty());
        assert_eq!(ctx.result.details, json!({"nested":{"new":2}}));
        assert!(ctx.is_error);
        assert_eq!(ctx.result.terminate, Some(true));
        Box::pin(async {
            Ok(AfterToolCallResult {
                is_error: Some(false),
                ..Default::default()
            })
        })
    });
    let third: AfterToolCall = Arc::new(|ctx, _| {
        assert!(!ctx.is_error);
        assert!(ctx.result.content.is_empty());
        assert_eq!(ctx.result.details, json!({"nested":{"new":2}}));
        Box::pin(async {
            Ok(AfterToolCallResult {
                details: Some(json!(null)),
                terminate: Some(false),
                ..Default::default()
            })
        })
    });
    let (agent, provider) = setup(
        vec![
            Script::Steps(call_steps(
                &[("id", "work", json!({}))],
                StopReason::ToolUse,
            )),
            Script::Steps(steps("end")),
        ],
        AgentOptions {
            tools: vec![t],
            after_tool_call: vec![first, second, third],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
    let r = results(&records);
    assert!(r[0].content.is_empty());
    assert_eq!(r[0].details, Some(json!(null)));
    assert!(!r[0].is_error);
    assert_eq!(provider.calls().len(), 2);
    assert!(events.lock().unwrap().iter().any(
        |e| matches!(e,AgentEvent::ToolExecutionEnd {result,..} if result.terminate == Some(false))
    ));
}

#[tokio::test]
async fn execution_and_after_hook_failures_become_final_error_results() {
    for fail_after in [false, true] {
        let first: AfterToolCall = Arc::new(|ctx, _| {
            assert!(ctx.is_error);
            assert_eq!(ctx.result, output("execute failed"));
            Box::pin(async {
                Ok(AfterToolCallResult {
                    content: Some(output("recovered").content),
                    details: Some(json!({"kept":true})),
                    is_error: Some(false),
                    terminate: Some(true),
                })
            })
        });
        let mut hooks = vec![first];
        if fail_after {
            hooks.push(Arc::new(|ctx, _| {
                assert!(!ctx.is_error);
                assert_eq!(ctx.result.details, json!({"kept":true}));
                Box::pin(async { Err("after failed".into()) })
            }));
            hooks.push(Arc::new(|_, _| panic!("later after hook")));
        }
        let (agent, provider) = setup(
            vec![
                Script::Steps(call_steps(
                    &[("id", "work", json!({}))],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![tool(
                    "work",
                    Arc::new(|_| Box::pin(async { Err("execute failed".into()) })),
                )],
                after_tool_call: hooks,
                ..Default::default()
            },
        );
        let events = capture(&agent);
        let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
        let r = results(&records);
        assert_eq!(r[0].is_error, fail_after);
        assert_eq!(
            r[0].content,
            output(if fail_after {
                "after failed"
            } else {
                "recovered"
            })
            .content
        );
        assert_eq!(
            r[0].details,
            Some(if fail_after {
                json!({})
            } else {
                json!({"kept":true})
            })
        );
        if fail_after {
            let Message::ToolResult(projected) = &provider.calls()[1].context.messages[2] else {
                panic!()
            };
            assert!(projected.is_error);
            assert_eq!(projected.content, r[0].content);
        }
        assert!(events.lock().unwrap().iter().any(|e|matches!(e,AgentEvent::ToolExecutionEnd {result,..} if result.terminate == if fail_after {None} else {Some(true)})));
    }
}

#[tokio::test]
async fn hook_context_is_batch_snapshot_and_raw_calls_are_unchanged() {
    for mode in [ToolExecutionMode::Parallel, ToolExecutionMode::Sequential] {
        let before: BeforeToolCall = Arc::new(|mut ctx, _| {
            assert_eq!(ctx.context.messages.len(), 2);
            assert_eq!(ctx.args["n"], 42);
            assert_eq!(ctx.tool_call.arguments().unwrap()["n"], "raw");
            ctx.context.messages.clear();
            ctx.assistant_message.content.clear();
            Box::pin(async {
                Ok(BeforeToolCallResult {
                    args: Some(json!({"n":7}).as_object().unwrap().clone()),
                    ..Default::default()
                })
            })
        });
        let after: AfterToolCall = Arc::new(|mut ctx, _| {
            assert_eq!(ctx.context.messages.len(), 2);
            assert_eq!(ctx.args["n"], 7);
            assert_eq!(ctx.tool_call.arguments().unwrap()["n"], "raw");
            assert_eq!(ctx.assistant_message.content.len(), 2);
            assert_eq!(ctx.result.content, output("done").content);
            assert!(!ctx.is_error);
            ctx.context.messages.clear();
            Box::pin(async { Ok(AfterToolCallResult::default()) })
        });
        let mut t = tool(
            "work",
            Arc::new(|inv| {
                assert_eq!(inv.args["n"], 7);
                Box::pin(async { Ok(output("done")) })
            }),
        );
        t.declaration.parameters = json!({"type":"object","properties":{"n":{"type":"number"}}});
        t.prepare = Some(Arc::new(|mut args, _| {
            args.insert("n".into(), json!("42"));
            Box::pin(async move { Ok(args) })
        }));
        let (agent, _) = setup(
            vec![
                Script::Steps(call_steps(
                    &[
                        ("a", "work", json!({"n":"raw"})),
                        ("b", "work", json!({"n":"raw"})),
                    ],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![t],
                tool_execution: mode,
                before_tool_call: vec![before],
                after_tool_call: vec![after],
                ..Default::default()
            },
        );
        let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
        assert_eq!(records.len(), 5);
        let AgentMessage::Model(Message::Assistant(raw)) = &records[1] else {
            panic!()
        };
        for block in &raw.content {
            let AssistantContent::ToolCall(call) = block else {
                panic!()
            };
            assert_eq!(call.arguments().unwrap()["n"], "raw");
        }
    }
}
