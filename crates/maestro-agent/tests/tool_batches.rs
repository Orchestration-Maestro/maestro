mod support;
use maestro_agent::*;
use maestro_models::*;
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::*;

#[tokio::test]
async fn parallel_preflight_is_serial_and_finishes_before_launch() {
    let log = Arc::new(Mutex::new(vec![]));
    let a = log.clone();
    let b = log.clone();
    let c = log.clone();
    let (wait, entered, release) = gate();
    let wait = Arc::new(Mutex::new(Some(wait)));
    let mut t = tool(
        "work",
        Arc::new(move |inv| {
            let log = c.clone();
            Box::pin(async move {
                assert!(log.lock().unwrap().contains(&"before-b".to_string()));
                log.lock()
                    .unwrap()
                    .push(format!("execute-{}", inv.tool_call_id));
                Ok(output("done"))
            })
        }),
    );
    t.prepare = Some(Arc::new(move |args, _| {
        let id = args["id"].as_str().unwrap().to_string();
        a.lock().unwrap().push(format!("prepare-{id}"));
        let wait = if id == "b" {
            wait.lock().unwrap().take()
        } else {
            None
        };
        Box::pin(async move {
            if let Some(ScriptStep::Wait(wait)) = wait {
                wait.await;
            }
            Ok(args)
        })
    }));
    let before: BeforeToolCall = Arc::new(move |ctx, _| {
        b.lock()
            .unwrap()
            .push(format!("before-{}", ctx.tool_call.id));
        Box::pin(async { Ok(BeforeToolCallResult::default()) })
    });
    let (agent, _) = setup(
        vec![
            Script::Steps(call_steps(
                &[
                    ("a", "work", json!({"id":"a"})),
                    ("reject", "absent", json!({})),
                    ("b", "work", json!({"id":"b"})),
                ],
                StopReason::ToolUse,
            )),
            Script::Steps(steps("end")),
        ],
        AgentOptions {
            tools: vec![t],
            before_tool_call: vec![before],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let mut run = agent.prompt(user("go"), options()).unwrap();
    tokio::select! { result = &mut run => panic!("execution passed preflight barrier: {result:?}"), result = entered => result.unwrap() };
    assert_eq!(
        *log.lock().unwrap(),
        vec!["prepare-a", "before-a", "prepare-b"]
    );
    assert!(events.lock().unwrap().iter().any(
        |e| matches!(e,AgentEvent::ToolExecutionEnd {tool_call_id,..} if tool_call_id=="reject")
    ));
    assert!(!events.lock().unwrap().iter().any(|e| matches!(
        e,
        AgentEvent::MessageEnd {
            message: AgentMessage::Model(Message::ToolResult(_))
        }
    )));
    release.send(()).unwrap();
    let records = run.await.unwrap();
    assert_eq!(
        results(&records)
            .iter()
            .map(|r| r.tool_call_id.as_str())
            .collect::<Vec<_>>(),
        vec!["a", "reject", "b"]
    );
}

#[tokio::test]
async fn parallel_execution_ends_in_completion_order_results_in_source_order() {
    for explicit in [false, true] {
        let (entered_tx, mut entered) = tokio::sync::mpsc::unbounded_channel();
        let (a_tx, a_rx) = tokio::sync::oneshot::channel();
        let (b_tx, b_rx) = tokio::sync::oneshot::channel();
        let gates = Arc::new(Mutex::new(vec![Some(a_rx), Some(b_rx)]));
        let mut t = tool(
            "work",
            Arc::new(move |inv| {
                let index = if inv.tool_call_id == "z" { 0 } else { 1 };
                let gate = gates.lock().unwrap()[index].take().unwrap();
                let entered = entered_tx.clone();
                Box::pin(async move {
                    entered.send(inv.tool_call_id.clone()).unwrap();
                    gate.await.unwrap();
                    Ok(output(&inv.tool_call_id))
                })
            }),
        );
        if explicit {
            t.execution_mode = Some(ToolExecutionMode::Parallel);
        }
        let (agent, _) = setup(
            vec![
                Script::Steps(call_steps(
                    &[("z", "work", json!({})), ("a", "work", json!({}))],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![t],
                tool_execution: ToolExecutionMode::Parallel,
                ..Default::default()
            },
        );
        let events = capture(&agent);
        let (end_tx, mut end_rx) = tokio::sync::mpsc::unbounded_channel();
        agent.subscribe(Arc::new(move |e, _| {
            if let AgentEvent::ToolExecutionEnd { tool_call_id, .. } = e {
                end_tx.send(tool_call_id).unwrap();
            }
            Box::pin(async {})
        }));
        let run = agent.prompt(user("go"), options()).unwrap();
        let launched = [entered.recv().await.unwrap(), entered.recv().await.unwrap()];
        assert!(launched.contains(&"a".to_string()));
        assert!(launched.contains(&"z".to_string()));
        b_tx.send(()).unwrap();
        assert_eq!(end_rx.recv().await.unwrap(), "a");
        assert!(!events.lock().unwrap().iter().any(|e| matches!(
            e,
            AgentEvent::MessageStart {
                message: AgentMessage::Model(Message::ToolResult(_))
            }
        )));
        a_tx.send(()).unwrap();
        let records = run.await.unwrap();
        assert_eq!(end_rx.recv().await.unwrap(), "z");
        let r = results(&records);
        assert_eq!(
            r.iter()
                .map(|r| r.tool_call_id.as_str())
                .collect::<Vec<_>>(),
            vec!["z", "a"]
        );
        let events = events.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter_map(|e| match e {
                    AgentEvent::MessageEnd {
                        message: AgentMessage::Model(Message::ToolResult(r)),
                    } => Some(r.tool_call_id.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec!["z", "a"]
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e,AgentEvent::TurnEnd {tool_results,..} if tool_results==&r))
        );
    }
}

#[tokio::test]
async fn any_sequential_setting_serializes_the_entire_batch() {
    for case in ["global", "repeated", "mixed", "rejected"] {
        let make = |name: &str| {
            let mut t = tool(
                name,
                Arc::new(|inv| {
                    (inv.progress)(output("partial"));
                    Box::pin(async { Ok(output("done")) })
                }),
            );
            t.prepare = Some(Arc::new(|args, _| Box::pin(async move { Ok(args) })));
            t
        };
        let mut first = make("first");
        let mut second = make("second");
        first.execution_mode = Some(if case == "repeated" {
            ToolExecutionMode::Sequential
        } else {
            ToolExecutionMode::Parallel
        });
        second.execution_mode = Some(if case == "global" {
            ToolExecutionMode::Parallel
        } else {
            ToolExecutionMode::Sequential
        });
        if case == "rejected" {
            second.declaration.parameters = json!(false);
        }
        let second_name = if case == "repeated" {
            "first"
        } else {
            "second"
        };
        let (agent, _) = setup(
            vec![
                Script::Steps(call_steps(
                    &[("a", "first", json!({})), ("b", second_name, json!({}))],
                    StopReason::ToolUse,
                )),
                Script::Steps(steps("end")),
            ],
            AgentOptions {
                tools: vec![first, second],
                tool_execution: if case == "global" {
                    ToolExecutionMode::Sequential
                } else {
                    ToolExecutionMode::Parallel
                },
                after_tool_call: vec![Arc::new(|_, _| {
                    Box::pin(async { Ok(AfterToolCallResult::default()) })
                })],
                ..Default::default()
            },
        );
        let events = capture(&agent);
        agent.prompt(user("go"), options()).unwrap().await.unwrap();
        let events = events.lock().unwrap();
        let relevant = events
            .iter()
            .filter_map(|e| match e {
                AgentEvent::ToolExecutionStart { tool_call_id, .. } => {
                    Some(format!("start-{tool_call_id}"))
                }
                AgentEvent::ToolExecutionUpdate { tool_call_id, .. } => {
                    Some(format!("update-{tool_call_id}"))
                }
                AgentEvent::ToolExecutionEnd { tool_call_id, .. } => {
                    Some(format!("end-{tool_call_id}"))
                }
                AgentEvent::MessageEnd {
                    message: AgentMessage::Model(Message::ToolResult(r)),
                } => Some(format!("record-{}", r.tool_call_id)),
                _ => None,
            })
            .collect::<Vec<_>>();
        let mut expected = vec!["start-a", "update-a", "end-a", "record-a", "start-b"];
        if case != "rejected" {
            expected.push("update-b");
        }
        expected.extend(["end-b", "record-b"]);
        assert_eq!(relevant, expected, "{case}");
    }
}

#[tokio::test]
async fn steering_waits_for_batch_and_follow_up_waits_for_tool_continuation() {
    for mode in [ToolExecutionMode::Parallel, ToolExecutionMode::Sequential] {
        for queued in [false, true] {
            let (wait, entered, release) = gate();
            let wait = Arc::new(Mutex::new(Some(wait)));
            let t = tool(
                "work",
                Arc::new(move |inv| {
                    let wait = if inv.tool_call_id == "a" {
                        wait.lock().unwrap().take()
                    } else {
                        None
                    };
                    Box::pin(async move {
                        if let Some(ScriptStep::Wait(wait)) = wait {
                            wait.await;
                        }
                        Ok(output(&inv.tool_call_id))
                    })
                }),
            );
            let (agent, provider) = setup(
                vec![
                    Script::Steps(call_steps(
                        &[("a", "work", json!({})), ("b", "work", json!({}))],
                        StopReason::ToolUse,
                    )),
                    Script::Steps(steps("continuation")),
                    Script::Steps(steps("followed")),
                ],
                AgentOptions {
                    tools: vec![t],
                    tool_execution: mode,
                    ..Default::default()
                },
            );
            let run = agent.prompt(user("go"), options()).unwrap();
            entered.await.unwrap();
            if queued {
                agent.steer(user("steering"));
            }
            agent.follow_up(user("follow"));
            assert_eq!(provider.calls().len(), 1);
            release.send(()).unwrap();
            let records = run.await.unwrap();
            assert_eq!(provider.calls().len(), 3);
            let calls = provider.calls();
            assert_eq!(calls[1].context.messages.len(), if queued { 5 } else { 4 });
            assert!(
                matches!(&calls[1].context.messages[2],Message::ToolResult(r) if r.tool_call_id=="a")
            );
            assert!(
                matches!(&calls[1].context.messages[3],Message::ToolResult(r) if r.tool_call_id=="b")
            );
            if queued {
                assert_eq!(
                    AgentMessage::Model(calls[1].context.messages[4].clone()),
                    user("steering")
                );
            }
            assert_eq!(
                AgentMessage::Model(calls[2].context.messages.last().unwrap().clone()),
                user("follow")
            );
            assert_eq!(results(&records).len(), 2);
        }
    }
}

#[tokio::test]
async fn termination_uses_all_finalized_results_and_stop_hook_wins() {
    for mode in [ToolExecutionMode::Parallel, ToolExecutionMode::Sequential] {
        for case in [
            "all", "false", "absent", "error", "set", "clear", "queues", "stop",
        ] {
            let t = tool(
                "work",
                Arc::new(move |inv| {
                    Box::pin(async move {
                        if case == "error" && inv.tool_call_id == "b" {
                            return Err("failed".into());
                        }
                        let mut r = output("done");
                        r.terminate = if case == "set" {
                            None
                        } else if inv.tool_call_id == "a"
                            || !["false", "absent", "stop"].contains(&case)
                        {
                            Some(true)
                        } else if case == "false" {
                            Some(false)
                        } else {
                            None
                        };
                        Ok(r)
                    })
                }),
            );
            let hooks = if case == "set" || case == "clear" {
                vec![Arc::new(move |_, _| {
                    Box::pin(async move {
                        Ok(AfterToolCallResult {
                            terminate: Some(case == "set"),
                            ..Default::default()
                        })
                    })
                        as std::pin::Pin<
                            Box<
                                dyn std::future::Future<
                                        Output = Result<AfterToolCallResult, String>,
                                    > + Send,
                            >,
                        >
                }) as AfterToolCall]
            } else {
                vec![]
            };
            let (wait, entered, release) = gate();
            let mut script = call_steps(
                &[("a", "work", json!({})), ("b", "work", json!({}))],
                StopReason::ToolUse,
            );
            script.insert(script.len() - 1, wait);
            let stopped = Arc::new(Mutex::new(false));
            let observed = stopped.clone();
            let stop = if case == "stop" {
                Some(Arc::new(move |ctx: StopAfterTurnContext| {
                    assert_eq!(ctx.tool_results.len(), 2);
                    assert_eq!(ctx.context.messages.len(), 4);
                    assert_eq!(ctx.new_messages.len(), 4);
                    *observed.lock().unwrap() = true;
                    Box::pin(async { true })
                        as std::pin::Pin<Box<dyn std::future::Future<Output = bool> + Send>>
                }) as StopAfterTurn)
            } else {
                None
            };
            let (agent, provider) = setup(
                vec![
                    Script::Steps(script),
                    Script::Steps(steps("next")),
                    Script::Steps(steps("follow")),
                ],
                AgentOptions {
                    tools: vec![t],
                    tool_execution: mode,
                    after_tool_call: hooks,
                    stop_after_turn: stop,
                    ..Default::default()
                },
            );
            let events = capture(&agent);
            let run = agent.prompt(user("go"), options()).unwrap();
            entered.await.unwrap();
            if case == "queues" || case == "stop" {
                agent.steer(user("steer"));
                agent.follow_up(user("follow"));
            }
            release.send(()).unwrap();
            run.await.unwrap();
            assert_eq!(
                provider.calls().len(),
                match case {
                    "all" | "set" | "stop" => 1,
                    "queues" => 3,
                    _ => 2,
                },
                "{case}"
            );
            if case == "stop" {
                assert!(*stopped.lock().unwrap());
                assert_eq!(agent.queue(Queue::Steering), vec![user("steer")]);
                assert_eq!(agent.queue(Queue::FollowUp), vec![user("follow")]);
                assert!(events.lock().unwrap().iter().any(
                    |e| matches!(e,AgentEvent::TurnEnd {tool_results,..} if tool_results.len()==2)
                ));
            }
            if case == "queues" {
                let calls = provider.calls();
                assert_eq!(
                    AgentMessage::Model(calls[1].context.messages.last().unwrap().clone()),
                    user("steer")
                );
                assert_eq!(
                    AgentMessage::Model(calls[2].context.messages.last().unwrap().clone()),
                    user("follow")
                );
            }
        }
        let (agent, provider) = setup(
            vec![Script::Steps(steps("empty"))],
            AgentOptions {
                tool_execution: mode,
                ..Default::default()
            },
        );
        agent.prompt(user("go"), options()).unwrap().await.unwrap();
        assert_eq!(provider.calls().len(), 1);
    }
}
