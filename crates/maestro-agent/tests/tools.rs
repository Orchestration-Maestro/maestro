mod support;
use maestro_agent::*;
use maestro_models::*;
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::*;

#[tokio::test]
async fn tool_turn_invokes_and_projects_finalized_results() {
    let invoked = Arc::new(Mutex::new(vec![]));
    let seen = invoked.clone();
    let final_output = ToolResult {
        content: vec![
            InputContent::Text(TextContent {
                text: "done".into(),
                replay_metadata: None,
            }),
            InputContent::Image(ImageContent {
                data: "AA==".into(),
                mime_type: "image/png".into(),
            }),
        ],
        details: json!({"private":42}),
        terminate: None,
    };
    let value = final_output.clone();
    let mut executable = tool(
        "work",
        Arc::new(move |invocation| {
            seen.lock()
                .unwrap()
                .push((invocation.tool_call_id.clone(), invocation.args.clone()));
            assert!(!invocation.cancellation.is_cancelled());
            (invocation.progress)(value.clone());
            let value = value.clone();
            Box::pin(async move { Ok(value) })
        }),
    );
    executable.declaration.parameters =
        json!({"type":"object","properties":{"n":{"type":"number"}},"required":["n"]});
    executable.prepare = Some(Arc::new(|mut args, _| {
        Box::pin(async move {
            args.insert("n".into(), json!("42"));
            Ok(args)
        })
    }));
    let mut script = vec![
        ScriptStep::Update(ProviderUpdate::ThinkingStart {
            content_index: 0,
            signature: Some("signature".into()),
        }),
        ScriptStep::Update(ProviderUpdate::ThinkingDelta {
            content_index: 0,
            delta: "reasoning".into(),
        }),
        ScriptStep::Update(ProviderUpdate::ThinkingEnd { content_index: 0 }),
        ScriptStep::Update(ProviderUpdate::TextStart { content_index: 1 }),
        ScriptStep::Update(ProviderUpdate::TextDelta {
            content_index: 1,
            delta: "working".into(),
        }),
        ScriptStep::Update(ProviderUpdate::TextEnd {
            content_index: 1,
            replay_metadata: Some("text-replay".into()),
        }),
        ScriptStep::Update(ProviderUpdate::ToolCallStart {
            content_index: 2,
            id: "id".into(),
            name: "work".into(),
            replay_metadata: Some("tool-replay".into()),
        }),
        ScriptStep::Update(ProviderUpdate::ToolCallDelta {
            content_index: 2,
            delta: "{}".into(),
        }),
        ScriptStep::Update(ProviderUpdate::ToolCallEnd { content_index: 2 }),
    ];
    script.push(ScriptStep::Update(ProviderUpdate::Done {
        reason: StopReason::ToolUse,
    }));
    let (agent, provider) = setup(
        vec![Script::Steps(script), Script::Steps(steps("finished"))],
        AgentOptions {
            tools: vec![executable.clone()],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
    assert_eq!(
        *invoked.lock().unwrap(),
        vec![("id".into(), json!({"n":42}).as_object().unwrap().clone())]
    );
    assert_eq!(results(&records)[0].details, Some(json!({"private":42})));
    assert!(!results(&records)[0].is_error);
    assert_eq!(provider.calls().len(), 2);
    assert_eq!(
        provider.calls()[0].context.tools,
        vec![executable.declaration]
    );
    let Message::ToolResult(projected) = &provider.calls()[1].context.messages[2] else {
        panic!()
    };
    assert_eq!(projected.details, None);
    assert_eq!(projected.content, final_output.content);
    assert_eq!(agent.state().context.messages, records);
    let events = events.lock().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, AgentEvent::MessageStart { .. }))
            .count(),
        4
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, AgentEvent::MessageEnd { .. }))
            .count(),
        4
    );
    let updates: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            AgentEvent::MessageUpdate {
                message,
                assistant_message_event,
            } => {
                let (label, partial) = match assistant_message_event {
                    ModelEvent::ThinkingStart { partial, .. } => ("thinking-start", partial),
                    ModelEvent::ThinkingDelta { partial, .. } => ("thinking-delta", partial),
                    ModelEvent::ThinkingEnd { partial, .. } => ("thinking-end", partial),
                    ModelEvent::TextStart { partial, .. } => ("text-start", partial),
                    ModelEvent::TextDelta { partial, .. } => ("text-delta", partial),
                    ModelEvent::TextEnd { partial, .. } => ("text-end", partial),
                    ModelEvent::ToolCallStart { partial, .. } => ("call-start", partial),
                    ModelEvent::ToolCallDelta { partial, .. } => ("call-delta", partial),
                    ModelEvent::ToolCallEnd { partial, .. } => ("call-end", partial),
                    _ => panic!("unexpected update"),
                };
                assert_eq!(message, partial);
                if label == "call-end" {
                    let AssistantContent::ToolCall(call) = &partial.content[2] else {
                        panic!()
                    };
                    assert_eq!(call.replay_metadata.as_deref(), Some("tool-replay"));
                    assert!(call.arguments().unwrap().is_empty());
                }
                Some(label)
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        &updates[..9],
        &[
            "thinking-start",
            "thinking-delta",
            "thinking-end",
            "text-start",
            "text-delta",
            "text-end",
            "call-start",
            "call-delta",
            "call-end"
        ]
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolExecutionStart { .. }))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolExecutionEnd { .. }))
            .count(),
        1
    );
    assert!(events.iter().any(|e| matches!(e, AgentEvent::ToolExecutionUpdate { args, partial_result, .. } if args.is_empty() && partial_result == &final_output)));
    assert!(events.iter().any(|e| matches!(e, AgentEvent::TurnEnd { tool_results, .. } if tool_results == &results(&records))));
}

#[tokio::test]
async fn tool_lookup_is_exact_and_first_match() {
    let first = tool(
        "work",
        Arc::new(|_| Box::pin(async { Ok(output("first")) })),
    );
    let mut duplicate = tool("work", Arc::new(|_| panic!("duplicate executed")));
    duplicate.declaration.parameters = json!(false);
    let (agent, _) = setup(
        vec![
            Script::Steps(call_steps(
                &[
                    ("a", "work", json!({})),
                    ("b", "Work", json!({})),
                    ("c", " work", json!({})),
                    ("d", "wórk", json!({})),
                ],
                StopReason::ToolUse,
            )),
            Script::Steps(steps("end")),
        ],
        AgentOptions {
            tools: vec![first, duplicate],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
    let r = results(&records);
    assert_eq!(r[0].content, output("first").content);
    for (result, name) in r[1..].iter().zip(["Work", " work", "wórk"]) {
        assert!(result.is_error);
        assert_eq!(
            result.content,
            output(&format!("Tool {name} not found")).content
        );
    }
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolExecutionStart { .. }))
            .count(),
        4
    );
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(e, AgentEvent::ToolExecutionEnd { .. }))
            .count(),
        4
    );
}

#[tokio::test]
async fn preparation_precedes_validation_and_preserves_raw_call() {
    let order = Arc::new(Mutex::new(vec![]));
    let a = order.clone();
    let b = order.clone();
    let c = order.clone();
    let mut t = tool(
        "work",
        Arc::new(move |inv| {
            c.lock().unwrap().push("execute");
            assert_eq!(inv.args["n"], 42);
            (inv.progress)(output("update"));
            Box::pin(async { Ok(output("done")) })
        }),
    );
    t.declaration.parameters =
        json!({"type":"object","properties":{"n":{"type":"number"}},"required":["n"]});
    t.prepare = Some(Arc::new(move |mut args, _| {
        a.lock().unwrap().push("prepare");
        args.insert("n".into(), json!("42"));
        Box::pin(async move { Ok(args) })
    }));
    let hook: BeforeToolCall = Arc::new(move |ctx, _| {
        b.lock().unwrap().push("before");
        assert_eq!(ctx.args["n"], 42);
        assert_eq!(ctx.tool_call.arguments().unwrap()["n"], "raw");
        Box::pin(async { Ok(BeforeToolCallResult::default()) })
    });
    let (agent, _) = setup(
        vec![
            Script::Steps(call_steps(
                &[("id", "work", json!({"n":"raw"}))],
                StopReason::ToolUse,
            )),
            Script::Steps(steps("end")),
        ],
        AgentOptions {
            tools: vec![t],
            before_tool_call: vec![hook],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
    assert_eq!(*order.lock().unwrap(), vec!["prepare", "before", "execute"]);
    let AgentMessage::Model(Message::Assistant(raw)) = &records[1] else {
        panic!()
    };
    let AssistantContent::ToolCall(call) = &raw.content[0] else {
        panic!()
    };
    assert_eq!(call.arguments().unwrap()["n"], "raw");
    for event in events.lock().unwrap().iter() {
        match event {
            AgentEvent::ToolExecutionStart { args, .. }
            | AgentEvent::ToolExecutionUpdate { args, .. } => assert_eq!(args["n"], "raw"),
            _ => {}
        }
    }
}

#[tokio::test]
async fn successful_terminal_calls_execute_but_partial_or_failed_calls_do_not() {
    for reason in [StopReason::Stop, StopReason::Length, StopReason::ToolUse] {
        let executed = Arc::new(Mutex::new(0));
        let count = executed.clone();
        let t = tool(
            "work",
            Arc::new(move |_| {
                *count.lock().unwrap() += 1;
                Box::pin(async { Ok(output("done")) })
            }),
        );
        let mut script = call_steps(&[("id", "work", json!({}))], reason);
        let (wait, entered, release) = gate();
        script.insert(script.len() - 1, wait);
        let (agent, provider) = setup(
            vec![Script::Steps(script), Script::Steps(steps("end"))],
            AgentOptions {
                tools: vec![t],
                ..Default::default()
            },
        );
        let run = agent.prompt(user("go"), options()).unwrap();
        entered.await.unwrap();
        assert_eq!(*executed.lock().unwrap(), 0);
        release.send(()).unwrap();
        run.await.unwrap();
        assert_eq!(*executed.lock().unwrap(), 1);
        assert_eq!(provider.calls().len(), 2);
    }
    for failure in [Failure::Cancelled, Failure::Transport] {
        let mut script = call_steps(&[("id", "work", json!({}))], StopReason::ToolUse);
        *script.last_mut().unwrap() = ScriptStep::Update(ProviderUpdate::Error { failure });
        let (agent, _) = setup(
            vec![Script::Steps(script)],
            AgentOptions {
                tools: vec![tool(
                    "work",
                    Arc::new(|_| panic!("failed assistant executed")),
                )],
                ..Default::default()
            },
        );
        assert!(results(&agent.prompt(user("go"), options()).unwrap().await.unwrap()).is_empty());
    }
    let script = vec![
        ScriptStep::Update(ProviderUpdate::ToolCallStart {
            content_index: 0,
            id: "id".into(),
            name: "work".into(),
            replay_metadata: None,
        }),
        ScriptStep::Update(ProviderUpdate::ToolCallDelta {
            content_index: 0,
            delta: "{\"n\":".into(),
        }),
        ScriptStep::Update(ProviderUpdate::ToolCallEnd { content_index: 0 }),
        ScriptStep::Update(ProviderUpdate::Done {
            reason: StopReason::Length,
        }),
    ];
    let (agent, _) = setup(
        vec![Script::Steps(script)],
        AgentOptions {
            tools: vec![tool("work", Arc::new(|_| panic!("partial executed")))],
            ..Default::default()
        },
    );
    assert!(results(&agent.prompt(user("go"), options()).unwrap().await.unwrap()).is_empty());
}

#[tokio::test]
async fn continue_from_tool_result_does_not_reexecute_history() {
    let (first, _) = setup(
        vec![Script::Steps(call_steps(
            &[("id", "work", json!({}))],
            StopReason::ToolUse,
        ))],
        AgentOptions {
            tools: vec![tool(
                "work",
                Arc::new(|_| {
                    Box::pin(async {
                        let mut r = output("done");
                        r.terminate = Some(true);
                        Ok(r)
                    })
                }),
            )],
            ..Default::default()
        },
    );
    let history = first.prompt(user("go"), options()).unwrap().await.unwrap();
    let (agent, provider) = setup(
        vec![Script::Steps(steps("end"))],
        AgentOptions {
            context: AgentContext {
                system_prompt: None,
                messages: history.clone(),
            },
            tools: vec![tool("work", Arc::new(|_| panic!("history replayed")))],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let new = agent.continue_run(options()).unwrap().await.unwrap();
    assert_eq!(new.len(), 1);
    assert_eq!(provider.calls()[0].context.messages.len(), 3);
    assert_eq!(&agent.state().context.messages[..3], history);
    assert!(
        !events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, AgentEvent::ToolExecutionStart { .. }))
    );
}

#[tokio::test]
async fn executable_tool_adapters_swap_without_caller_changes() {
    async fn scenario(
        t: Tool,
        release: Option<tokio::sync::oneshot::Sender<()>>,
        entered: Option<tokio::sync::oneshot::Receiver<()>>,
    ) {
        let (agent, provider) = setup(
            vec![
                Script::Steps(call_steps(
                    &[("id", "work", json!({"n":42}))],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![t],
                ..Default::default()
            },
        );
        let run = agent.prompt(user("go"), options()).unwrap();
        if let Some(entered) = entered {
            entered.await.unwrap();
            release.unwrap().send(()).unwrap();
        }
        let records = run.await.unwrap();
        assert_eq!(results(&records)[0].content, output("done").content);
        assert!(!results(&records)[0].is_error);
        assert_eq!(provider.calls().len(), 2);
        assert_eq!(agent.state().context.messages, records);
    }
    let immediate = tool(
        "work",
        Arc::new(|inv| {
            assert_eq!(inv.tool_call_id, "id");
            assert_eq!(inv.args["n"], 42);
            Box::pin(async { Ok(output("done")) })
        }),
    );
    scenario(immediate, None, None).await;
    let (wait, entered, release) = gate();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let controlled = tool(
        "work",
        Arc::new(move |inv| {
            assert_eq!(inv.tool_call_id, "id");
            assert_eq!(inv.args["n"], 42);
            let wait = wait.lock().unwrap().take().unwrap();
            Box::pin(async move {
                if let ScriptStep::Wait(wait) = wait {
                    wait.await;
                }
                Ok(output("done"))
            })
        }),
    );
    scenario(controlled, Some(release), Some(entered)).await;
}

#[tokio::test]
async fn tool_validation_preserves_shared_scalar_semantics() {
    for (input, kind, expected) in [
        (json!("\u{feff}42\u{feff}"), "number", Some(json!(42))),
        (json!("\u{0085}42\u{0085}"), "number", None),
        (json!(1.0), "string", Some(json!("1"))),
        (json!(-0.0), "string", Some(json!("0"))),
        (json!(1e-7), "string", Some(json!("1e-7"))),
        (json!(1e-6), "string", Some(json!("0.000001"))),
        (json!(1e20), "string", Some(json!("100000000000000000000"))),
        (json!(1e21), "string", Some(json!("1e+21"))),
        (
            json!(9007199254740993u64),
            "string",
            Some(json!("9007199254740992")),
        ),
    ] {
        let execute_expected = expected.clone();
        let invoked = Arc::new(Mutex::new(false));
        let seen = invoked.clone();
        let mut t = tool(
            "work",
            Arc::new(move |inv| {
                *seen.lock().unwrap() = true;
                assert_eq!(Some(inv.args["n"].clone()), execute_expected);
                Box::pin(async { Ok(output("done")) })
            }),
        );
        t.declaration.parameters =
            json!({"type":"object","properties":{"n":{"type":kind}},"required":["n"]});
        let (agent, _) = setup(
            vec![
                Script::Steps(call_steps(
                    &[("id", "work", json!({"n":input}))],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![t],
                ..Default::default()
            },
        );
        let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
        assert_eq!(*invoked.lock().unwrap(), expected.is_some());
        assert_eq!(results(&records)[0].is_error, expected.is_none());
    }
}
