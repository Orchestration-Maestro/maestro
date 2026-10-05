mod support;
use maestro_agent::*;
use maestro_models::*;
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::*;

#[tokio::test]
async fn abort_reaches_preparation_hooks_and_launched_tools() {
    for phase in ["prepare", "before", "execute", "after"] {
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let entered = Arc::new(Mutex::new(Some(entered_tx)));
        let signals = Arc::new(Mutex::new(vec![]));
        let seen = signals.clone();
        let notify = entered.clone();
        let mut t = tool(
            "work",
            Arc::new(move |inv| {
                seen.lock().unwrap().push(inv.cancellation.clone());
                let notify = notify.clone();
                Box::pin(async move {
                    if phase == "execute" {
                        notify.lock().unwrap().take().unwrap().send(()).unwrap();
                        inv.cancellation.cancelled().await;
                    }
                    let mut r = output("done");
                    r.terminate = Some(true);
                    Ok(r)
                })
            }),
        );
        let seen = signals.clone();
        let notify = entered.clone();
        t.prepare = Some(Arc::new(move |args, cancel| {
            seen.lock().unwrap().push(cancel.clone());
            let notify = notify.clone();
            Box::pin(async move {
                if phase == "prepare" {
                    notify.lock().unwrap().take().unwrap().send(()).unwrap();
                    cancel.cancelled().await;
                }
                Ok(args)
            })
        }));
        let seen = signals.clone();
        let notify = entered.clone();
        let before: BeforeToolCall = Arc::new(move |_, cancel| {
            seen.lock().unwrap().push(cancel.clone());
            let notify = notify.clone();
            Box::pin(async move {
                if phase == "before" {
                    notify.lock().unwrap().take().unwrap().send(()).unwrap();
                    cancel.cancelled().await;
                }
                Ok(BeforeToolCallResult::default())
            })
        });
        let seen = signals.clone();
        let notify = entered.clone();
        let after: AfterToolCall = Arc::new(move |_, cancel| {
            seen.lock().unwrap().push(cancel.clone());
            let notify = notify.clone();
            Box::pin(async move {
                if phase == "after" {
                    notify.lock().unwrap().take().unwrap().send(()).unwrap();
                    cancel.cancelled().await;
                }
                Ok(AfterToolCallResult::default())
            })
        });
        let (agent, _) = setup(
            vec![Script::Steps(call_steps(
                &[("id", "work", json!({}))],
                StopReason::ToolUse,
            ))],
            AgentOptions {
                tools: vec![t],
                before_tool_call: vec![before],
                after_tool_call: vec![after],
                ..Default::default()
            },
        );
        let request = options();
        let cancel = request.cancellation.clone();
        let run = agent.prompt(user("go"), request).unwrap();
        entered_rx.await.unwrap();
        assert!(!cancel.is_cancelled());
        agent.abort();
        run.await.unwrap();
        assert!(cancel.is_cancelled());
        assert_eq!(signals.lock().unwrap().len(), 4);
        assert!(
            signals
                .lock()
                .unwrap()
                .iter()
                .all(Cancellation::is_cancelled)
        );
        assert!(!agent.state().is_running);
    }
    let (launched_tx, mut launched) = tokio::sync::mpsc::unbounded_channel();
    let (late_tx, late_rx) = tokio::sync::oneshot::channel();
    let late = Arc::new(Mutex::new(Some(late_rx)));
    let t = tool(
        "work",
        Arc::new(move |inv| {
            let late = if inv.tool_call_id == "late" {
                late.lock().unwrap().take()
            } else {
                None
            };
            let launched = launched_tx.clone();
            Box::pin(async move {
                launched.send(()).unwrap();
                inv.cancellation.cancelled().await;
                if let Some(late) = late {
                    late.await.unwrap();
                }
                let mut r = output("settled");
                r.terminate = Some(true);
                Ok(r)
            })
        }),
    );
    let (agent, _) = setup(
        vec![Script::Steps(call_steps(
            &[("peer", "work", json!({})), ("late", "work", json!({}))],
            StopReason::ToolUse,
        ))],
        AgentOptions {
            tools: vec![t],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let (end_tx, end_rx) = tokio::sync::oneshot::channel();
    let end_tx = Arc::new(Mutex::new(Some(end_tx)));
    agent.subscribe(Arc::new(move |e, _| {
        if matches!(e,AgentEvent::ToolExecutionEnd {tool_call_id,..} if tool_call_id=="peer") {
            end_tx.lock().unwrap().take().unwrap().send(()).unwrap();
        }
        Box::pin(async {})
    }));
    let mut run = agent.prompt(user("go"), options()).unwrap();
    launched.recv().await.unwrap();
    launched.recv().await.unwrap();
    agent.abort();
    end_rx.await.unwrap();
    pending(&mut run).await;
    assert!(agent.state().is_running);
    assert!(
        !events
            .lock()
            .unwrap()
            .iter()
            .any(|e| matches!(e, AgentEvent::AgentEnd { .. }))
    );
    late_tx.send(()).unwrap();
    assert_eq!(results(&run.await.unwrap()).len(), 2);
}

#[tokio::test]
async fn accepted_progress_and_end_subscribers_precede_idle() {
    for failed in [false, true] {
        for aborted in [false, true] {
            let (update_tx, update_rx) = tokio::sync::oneshot::channel();
            let update_tx = Arc::new(Mutex::new(Some(update_tx)));
            let (release_tx, release_rx) = tokio::sync::oneshot::channel();
            let release_rx = Arc::new(Mutex::new(Some(release_rx)));
            let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
            let finished_tx = Arc::new(Mutex::new(Some(finished_tx)));
            let t = tool(
                "work",
                Arc::new(move |inv| {
                    let finished = finished_tx.clone();
                    Box::pin(async move {
                        (inv.progress)(output("one"));
                        (inv.progress)(output("two"));
                        finished.lock().unwrap().take().unwrap().send(()).unwrap();
                        if failed {
                            Err("failed".into())
                        } else {
                            let mut r = output("done");
                            r.terminate = Some(true);
                            Ok(r)
                        }
                    })
                }),
            );
            let after = Arc::new(Mutex::new(false));
            let after_seen = after.clone();
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
                    after_tool_call: vec![Arc::new(move |_, _| {
                        *after_seen.lock().unwrap() = true;
                        Box::pin(async { Ok(AfterToolCallResult::default()) })
                    })],
                    ..Default::default()
                },
            );
            let log = Arc::new(Mutex::new(vec![]));
            let a = log.clone();
            let update = update_tx.clone();
            let release = release_rx.clone();
            agent.subscribe(Arc::new(move |e,_| {let a=a.clone();let held=if matches!(&e,AgentEvent::ToolExecutionUpdate {partial_result,..} if partial_result.content==output("one").content) {update.lock().unwrap().take().unwrap().send(()).unwrap();release.lock().unwrap().take()} else {None};Box::pin(async move {if let Some(held)=held {held.await.unwrap();}a.lock().unwrap().push(format!("first-{}",label(&e)));})}));
            let b = log.clone();
            agent.subscribe(Arc::new(move |e, _| {
                b.lock().unwrap().push(format!("second-{}", label(&e)));
                Box::pin(async {})
            }));
            let (end_tx, end_rx) = tokio::sync::oneshot::channel();
            let end_tx = Arc::new(Mutex::new(Some(end_tx)));
            let (end_release, end_gate) = tokio::sync::oneshot::channel();
            let end_gate = Arc::new(Mutex::new(Some(end_gate)));
            agent.subscribe(Arc::new(move |e, _| {
                let gate = if matches!(e, AgentEvent::AgentEnd { .. }) {
                    end_tx.lock().unwrap().take().unwrap().send(()).unwrap();
                    end_gate.lock().unwrap().take()
                } else {
                    None
                };
                Box::pin(async move {
                    if let Some(gate) = gate {
                        gate.await.unwrap();
                    }
                })
            }));
            let events = capture(&agent);
            let mut run = agent.prompt(user("go"), options()).unwrap();
            update_rx.await.unwrap();
            finished_rx.await.unwrap();
            if aborted {
                agent.abort();
            }
            pending(&mut run).await;
            assert!(!*after.lock().unwrap());
            assert!(agent.state().is_running);
            assert!(!events.lock().unwrap().iter().any(|e| matches!(
                e,
                AgentEvent::ToolExecutionEnd { .. }
                    | AgentEvent::TurnEnd { .. }
                    | AgentEvent::AgentEnd { .. }
                    | AgentEvent::MessageEnd {
                        message: AgentMessage::Model(Message::ToolResult(_))
                    }
            )));
            release_tx.send(()).unwrap();
            end_rx.await.unwrap();
            assert!(*after.lock().unwrap());
            pending(&mut run).await;
            assert!(agent.state().is_running);
            drop(run);
            let mut idle = Box::pin(agent.wait_for_idle());
            pending(&mut idle).await;
            end_release.send(()).unwrap();
            idle.await.unwrap();
            assert!(!agent.state().is_running);
            let log = log.lock().unwrap();
            let (pairs, remainder) = log.as_chunks::<2>();
            assert!(remainder.is_empty());
            for pair in pairs {
                assert_eq!(
                    pair[0].strip_prefix("first-"),
                    pair[1].strip_prefix("second-")
                );
            }
            assert_eq!(
                log.iter()
                    .filter(|s| s.as_str() == "first-tool_update")
                    .count(),
                2
            );
        }
    }
}

#[tokio::test]
async fn pending_tool_calls_reduce_before_subscribers() {
    let (entered_tx, mut entered) = tokio::sync::mpsc::unbounded_channel();
    let (release_tx, release_rx) = tokio::sync::watch::channel(false);
    let t = tool(
        "work",
        Arc::new(move |inv| {
            let entered = entered_tx.clone();
            let mut release = release_rx.clone();
            Box::pin(async move {
                entered.send(inv.tool_call_id).unwrap();
                while !*release.borrow() {
                    release.changed().await.unwrap();
                }
                let mut r = output("done");
                r.terminate = Some(true);
                Ok(r)
            })
        }),
    );
    let (agent, _) = setup(
        vec![
            Script::Steps(call_steps(
                &[
                    ("z", "work", json!({})),
                    ("a", "work", json!({})),
                    ("reject", "missing", json!({})),
                ],
                StopReason::ToolUse,
            )),
            Script::Steps(steps("end")),
        ],
        AgentOptions {
            tools: vec![t],
            ..Default::default()
        },
    );
    let snapshots = Arc::new(Mutex::new(vec![]));
    let observed = snapshots.clone();
    let owner = agent.clone();
    let subscription = agent.subscribe(Arc::new(move |e, _| {
        if let AgentEvent::ToolExecutionStart { tool_call_id, .. }
        | AgentEvent::ToolExecutionEnd { tool_call_id, .. } = e
        {
            let mut state = owner.state();
            observed
                .lock()
                .unwrap()
                .push((tool_call_id, state.pending_tool_calls.clone()));
            state.pending_tool_calls.clear();
        }
        Box::pin(async {})
    }));
    let run = agent.prompt(user("go"), options()).unwrap();
    entered.recv().await.unwrap();
    entered.recv().await.unwrap();
    assert_eq!(agent.state().pending_tool_calls, vec!["z", "a"]);
    release_tx.send_replace(true);
    run.await.unwrap();
    let snapshots = snapshots.lock().unwrap().clone();
    assert_eq!(
        &snapshots[..4],
        &[
            ("z".into(), vec!["z".into()]),
            ("a".into(), vec!["z".into(), "a".into()]),
            (
                "reject".into(),
                vec!["z".into(), "a".into(), "reject".into()]
            ),
            ("reject".into(), vec!["z".into(), "a".into()])
        ]
    );
    assert!(
        snapshots[4..]
            .iter()
            .all(|(id, pending)| !pending.contains(id))
    );
    assert!(agent.state().pending_tool_calls.is_empty());
    agent.unsubscribe(subscription);
    drop(snapshots);

    let (agent, _) = setup(
        vec![Script::Steps(call_steps(
            &[("same", "work", json!({})), ("same", "work", json!({}))],
            StopReason::ToolUse,
        ))],
        AgentOptions {
            tools: vec![tool(
                "work",
                Arc::new(|_| {
                    Box::pin(async {
                        let mut result = output("done");
                        result.terminate = Some(true);
                        Ok(result)
                    })
                }),
            )],
            ..Default::default()
        },
    );
    let owner = agent.clone();
    let subscription = agent.subscribe(Arc::new(move |event, _| {
        if matches!(event, AgentEvent::ToolExecutionStart { .. }) {
            assert_eq!(owner.state().pending_tool_calls, vec!["same"]);
        }
        Box::pin(async {})
    }));
    assert_eq!(
        results(&agent.prompt(user("go"), options()).unwrap().await.unwrap()).len(),
        2
    );
    assert!(agent.state().pending_tool_calls.is_empty());
    agent.unsubscribe(subscription);
}

#[test]
fn tool_callbacks_and_cleanup_run_outside_state_locks() {
    struct Cleanup {
        owner: Arc<Mutex<Option<Agent>>>,
        dropped: Arc<Mutex<usize>>,
    }
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let owner = self.owner.lock().unwrap().clone();
            if let Some(owner) = owner {
                let _ = owner.state();
            }
            *self.dropped.lock().unwrap() += 1;
        }
    }
    let (completed, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            let owner = Arc::new(Mutex::new(None::<Agent>));
            let dropped = Arc::new(Mutex::new(0));
            let calls = Arc::new(Mutex::new(vec![]));
            let observe = |stage: &'static str| {
                let owner = owner.clone();
                let calls = calls.clone();
                move || {
                    assert!(owner.lock().unwrap().as_ref().unwrap().state().is_running);
                    calls.lock().unwrap().push(stage);
                }
            };
            let execute = observe("execute");
            let cleanup_owner = owner.clone();
            let cleanup_dropped = dropped.clone();
            let mut t = tool(
                "work",
                Arc::new(move |inv| {
                    execute();
                    let cleanup = Cleanup {
                        owner: cleanup_owner.clone(),
                        dropped: cleanup_dropped.clone(),
                    };
                    (inv.progress)(output("partial"));
                    Box::pin(async move {
                        let _cleanup = cleanup;
                        let mut r = output("done");
                        r.terminate = Some(true);
                        Ok(r)
                    })
                }),
            );
            let prepare = observe("prepare");
            t.prepare = Some(Arc::new(move |args, _| {
                prepare();
                Box::pin(async move { Ok(args) })
            }));
            let before = observe("before");
            let after = observe("after");
            let clock = observe("clock");
            let (agent, _) = setup(
                vec![Script::Steps(call_steps(
                    &[("id", "work", json!({}))],
                    StopReason::ToolUse,
                ))],
                AgentOptions {
                    tools: vec![t],
                    before_tool_call: vec![Arc::new(move |_, _| {
                        before();
                        Box::pin(async { Ok(BeforeToolCallResult::default()) })
                    })],
                    after_tool_call: vec![Arc::new(move |_, _| {
                        after();
                        Box::pin(async { Ok(AfterToolCallResult::default()) })
                    })],
                    clock: Some(Arc::new(move || {
                        clock();
                        0
                    })),
                    ..Default::default()
                },
            );
            *owner.lock().unwrap() = Some(agent.clone());
            let progress = observe("progress");
            let cleanup = Cleanup {
                owner: owner.clone(),
                dropped: dropped.clone(),
            };
            let subscription = agent.subscribe(Arc::new(move |e, _| {
                let _ = &cleanup;
                if matches!(e, AgentEvent::ToolExecutionUpdate { .. }) {
                    progress();
                }
                Box::pin(async {})
            }));
            agent.prompt(user("go"), options()).unwrap().await.unwrap();
            agent.unsubscribe(subscription);
            assert_eq!(
                *calls.lock().unwrap(),
                vec!["prepare", "before", "execute", "progress", "after", "clock"]
            );
            assert_eq!(*dropped.lock().unwrap(), 2);
            owner.lock().unwrap().take();
            completed.send(()).unwrap();
        });
    });
    completion
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("tool callbacks and cleanup must permit reentrant state observation");
    worker.join().unwrap();
}

#[tokio::test]
async fn tool_result_clock_samples_at_message_creation() {
    for mode in [ToolExecutionMode::Parallel, ToolExecutionMode::Sequential] {
        let ends = Arc::new(Mutex::new(vec![]));
        let sampled = Arc::new(Mutex::new(vec![]));
        let seen = ends.clone();
        let clock_samples = sampled.clone();
        let clock = Arc::new(move || {
            let mut samples = clock_samples.lock().unwrap();
            assert_eq!(
                seen.lock().unwrap().len(),
                if mode == ToolExecutionMode::Parallel {
                    2
                } else {
                    samples.len() + 1
                }
            );
            let value = samples.len() as u64;
            samples.push(value);
            value
        });
        let t = tool(
            "work",
            Arc::new(|_| {
                Box::pin(async {
                    let mut r = output("done");
                    r.terminate = Some(true);
                    Ok(r)
                })
            }),
        );
        let (agent, _) = setup(
            vec![Script::Steps(call_steps(
                &[("z", "work", json!({})), ("a", "work", json!({}))],
                StopReason::ToolUse,
            ))],
            AgentOptions {
                tools: vec![t],
                tool_execution: mode,
                clock: Some(clock),
                ..Default::default()
            },
        );
        let seen = ends.clone();
        agent.subscribe(Arc::new(move |e, _| {
            if let AgentEvent::ToolExecutionEnd { tool_call_id, .. } = e {
                seen.lock().unwrap().push(tool_call_id);
            }
            Box::pin(async {})
        }));
        let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
        let r = results(&records);
        assert_eq!(
            r.iter()
                .map(|r| (&*r.tool_call_id, r.timestamp))
                .collect::<Vec<_>>(),
            vec![("z", 0), ("a", 1)]
        );
        assert_eq!(*sampled.lock().unwrap(), vec![0, 1]);
    }
}

#[test]
fn progress_updates_deliver_without_waiting_for_earlier_updates() {
    let (completed, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        runtime.block_on(async {
            for failed in [false, true] {
                let t = tool(
                    "work",
                    Arc::new(move |inv| {
                        (inv.progress)(output("A"));
                        (inv.progress)(output("B"));
                        Box::pin(async move {
                            if failed {
                                Err("failed".into())
                            } else {
                                let mut result = output("done");
                                result.terminate = Some(true);
                                Ok(result)
                            }
                        })
                    }),
                );
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
                        ..Default::default()
                    },
                );
                let (signal, wait) = tokio::sync::oneshot::channel();
                let signal = Arc::new(Mutex::new(Some(signal)));
                let wait = Arc::new(Mutex::new(Some(wait)));
                let observed = Arc::new(Mutex::new(vec![]));
                let first = observed.clone();
                let observer = agent.clone();
                let subscription = agent.subscribe(Arc::new(move |event, _| {
                    let mut gate = None;
                    let mut label = None;
                    if let AgentEvent::ToolExecutionUpdate { partial_result, .. } = event {
                        assert!(observer.state().is_running);
                        if partial_result == output("A") {
                            gate = wait.lock().unwrap().take();
                            label = Some("first-A");
                        } else {
                            assert_eq!(partial_result, output("B"));
                            signal.lock().unwrap().take().unwrap().send(()).unwrap();
                            label = Some("first-B");
                        }
                    }
                    let first = first.clone();
                    Box::pin(async move {
                        if let Some(gate) = gate {
                            gate.await.unwrap();
                        }
                        if let Some(label) = label {
                            first.lock().unwrap().push(label);
                        }
                    })
                }));
                let second = observed.clone();
                agent.subscribe(Arc::new(move |event, _| {
                    let mut seen = second.lock().unwrap();
                    match event {
                        AgentEvent::ToolExecutionUpdate { partial_result, .. } => {
                            let (first, second) = if partial_result == output("A") {
                                ("first-A", "second-A")
                            } else {
                                assert_eq!(partial_result, output("B"));
                                ("first-B", "second-B")
                            };
                            assert!(seen.contains(&first));
                            seen.push(second);
                        }
                        AgentEvent::ToolExecutionEnd { .. } => {
                            assert!(seen.contains(&"second-A"));
                            assert!(seen.contains(&"second-B"));
                            seen.push("end");
                        }
                        _ => {}
                    }
                    Box::pin(async {})
                }));
                let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
                agent.unsubscribe(subscription);
                assert_eq!(results(&records)[0].is_error, failed);
                assert_eq!(observed.lock().unwrap().len(), 5);
                assert_eq!(observed.lock().unwrap().last(), Some(&"end"));
            }
            completed.send(()).unwrap();
        });
    });
    completion
        .recv_timeout(std::time::Duration::from_secs(2))
        .expect("progress B must deliver while progress A waits for its signal");
    worker.join().unwrap();
}

#[tokio::test]
async fn progress_from_a_plain_thread_is_delivered() {
    let t = tool(
        "work",
        Arc::new(move |inv| {
            Box::pin(async move {
                std::thread::spawn(move || (inv.progress)(output("partial")))
                    .join()
                    .expect("progress submission from a plain thread must not panic");
                let mut result = output("done");
                result.terminate = Some(true);
                Ok(result)
            })
        }),
    );
    let (agent, _) = setup(
        vec![Script::Steps(call_steps(
            &[("id", "work", json!({}))],
            StopReason::ToolUse,
        ))],
        AgentOptions {
            tools: vec![t],
            ..Default::default()
        },
    );
    let events = capture(&agent);
    let records = agent.prompt(user("go"), options()).unwrap().await.unwrap();
    assert_eq!(results(&records)[0].content, output("done").content);
    assert!(!results(&records)[0].is_error);
    let updates: Vec<_> = events
        .lock()
        .unwrap()
        .iter()
        .filter_map(|event| match event {
            AgentEvent::ToolExecutionUpdate { partial_result, .. } => Some(partial_result.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(updates, vec![output("partial")]);
    assert!(!agent.state().is_running);
}

#[tokio::test]
async fn submitted_progress_retains_subscription_snapshot() {
    let owner = Arc::new(Mutex::new(None::<(Agent, SubscriptionId)>));
    let seen = Arc::new(Mutex::new(vec![]));
    let executor_owner = owner.clone();
    let t = tool(
        "work",
        Arc::new(move |inv| {
            (inv.progress)(output("accepted"));
            let (agent, subscription) = executor_owner.lock().unwrap().as_ref().unwrap().clone();
            agent.unsubscribe(subscription);
            Box::pin(async {
                let mut r = output("done");
                r.terminate = Some(true);
                Ok(r)
            })
        }),
    );
    let (agent, _) = setup(
        vec![Script::Steps(call_steps(
            &[("id", "work", json!({}))],
            StopReason::ToolUse,
        ))],
        AgentOptions {
            tools: vec![t],
            ..Default::default()
        },
    );
    let observed = seen.clone();
    let subscription = agent.subscribe(Arc::new(move |event, _| {
        if let AgentEvent::ToolExecutionUpdate { partial_result, .. } = event {
            observed.lock().unwrap().push(partial_result);
        }
        Box::pin(async {})
    }));
    *owner.lock().unwrap() = Some((agent.clone(), subscription));
    agent.prompt(user("go"), options()).unwrap().await.unwrap();
    assert_eq!(*seen.lock().unwrap(), vec![output("accepted")]);
    owner.lock().unwrap().take();
}

#[tokio::test]
async fn panicking_executor_settles_accepted_progress_and_launched_peer() {
    let (entered_tx, mut entered) = tokio::sync::mpsc::unbounded_channel();
    let (release_tx, release_rx) = tokio::sync::oneshot::channel();
    let release = Arc::new(Mutex::new(Some(release_rx)));
    let t = tool(
        "work",
        Arc::new(move |inv| {
            let entered = entered_tx.clone();
            let gate = if inv.tool_call_id == "peer" {
                release.lock().unwrap().take()
            } else {
                None
            };
            Box::pin(async move {
                entered.send(inv.tool_call_id.clone()).unwrap();
                if let Some(gate) = gate {
                    gate.await.unwrap();
                    Ok(output("done"))
                } else {
                    (inv.progress)(output("accepted"));
                    panic!("contract violation")
                }
            })
        }),
    );
    let (agent, _) = setup(
        vec![Script::Steps(call_steps(
            &[("bad", "work", json!({})), ("peer", "work", json!({}))],
            StopReason::ToolUse,
        ))],
        AgentOptions {
            tools: vec![t],
            ..Default::default()
        },
    );
    let (update_tx, update_rx) = tokio::sync::oneshot::channel();
    let update_tx = Arc::new(Mutex::new(Some(update_tx)));
    let (update_release, update_gate) = tokio::sync::oneshot::channel();
    let update_gate = Arc::new(Mutex::new(Some(update_gate)));
    agent.subscribe(Arc::new(move |event, _| {
        let gate = if matches!(event, AgentEvent::ToolExecutionUpdate { .. }) {
            update_tx.lock().unwrap().take().unwrap().send(()).unwrap();
            update_gate.lock().unwrap().take()
        } else {
            None
        };
        Box::pin(async move {
            if let Some(gate) = gate {
                gate.await.unwrap();
            }
        })
    }));
    let mut run = agent.prompt(user("go"), options()).unwrap();
    entered.recv().await.unwrap();
    entered.recv().await.unwrap();
    update_rx.await.unwrap();
    pending(&mut run).await;
    assert!(agent.state().is_running);
    update_release.send(()).unwrap();
    pending(&mut run).await;
    release_tx.send(()).unwrap();
    assert_eq!(run.await, Err(AgentError::RunFailed));
    assert_eq!(agent.wait_for_idle().await, Err(AgentError::RunFailed));
    assert!(agent.state().pending_tool_calls.is_empty());
    assert!(!agent.state().is_running);
}
