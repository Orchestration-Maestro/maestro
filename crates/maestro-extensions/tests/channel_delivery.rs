#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod support;
use std::sync::Arc;
use support::{mark, run};
#[test]
fn empty_channels_are_inert() {
    run(|env| async move {
        for row in cases("empty_channels_are_inert") {
            env.bus
                .emit(row["input"].as_str().unwrap(), serde_json::Value::Null)
                .await;
            env.verify(row["id"].as_str().unwrap());
        }
    });
}

#[test]
fn channels_preserve_payload_values_and_order() {
    run(|env| async move {
        for row in cases("channels_preserve_payload_values_and_order") {
            env.bus.clear();
            env.trace.lock().unwrap().clear();
            let channel = row["input"]["channel"].as_str().unwrap();
            let trace = env.trace.clone();
            env.bus.on(
                channel,
                Arc::new(move |data, _| {
                    mark(&trace, data.as_ref().clone());
                    Ok(None)
                }),
            );
            let decoy = format!("{channel}x");
            env.bus
                .on(&decoy, Arc::new(|_, _| panic!("aliased channel")));
            env.bus.emit(channel, row["input"]["data"].clone()).await;
            env.verify(row["id"].as_str().unwrap());
            assert_eq!(
                env.trace.lock().unwrap()[0].to_string(),
                row["input"]["data"].to_string()
            );
        }
    });
}

#[test]
fn duplicate_registrations_unsubscribe_independently() {
    run(|env| async move {
        let trace = env.trace.clone();
        let handler: maestro_extensions::EventListener = Arc::new(move |data, _| {
            mark(&trace, data.as_ref().clone());
            Ok(None)
        });
        let first = env.bus.on("x", handler.clone());
        let second = env.bus.on("x", handler);
        env.bus.emit("x", 1.into()).await;
        first.unsubscribe();
        first.unsubscribe();
        env.bus.emit("x", 2.into()).await;
        second.unsubscribe();
        env.bus.emit("x", 3.into()).await;
        env.verify("duplicates");
    });
}

#[test]
fn dropping_a_subscription_handle_does_not_unsubscribe() {
    run(|env| async move {
        drop(env.bus.on("x", env.listener("first")));
        env.bus.emit("x", serde_json::Value::Null).await;
        env.bus.clear();
        env.bus.on("x", env.listener("second"));
        env.bus.emit("x", serde_json::Value::Null).await;
        env.bus.clear();
        env.bus.on("x", env.listener("after-clear"));
        env.bus.emit("x", serde_json::Value::Null).await;
        env.verify("ignored-unsubscribe");
    });
}

#[test]
fn clear_removes_all_channels_and_allows_reuse() {
    run(|env| async move {
        for channel in ["x", "y"] {
            let trace = env.trace.clone();
            env.bus.on(
                channel,
                Arc::new(move |data, _| {
                    mark(&trace, serde_json::json!([channel, *data]));
                    Ok(None)
                }),
            );
        }
        env.bus.emit("x", 1.into()).await;
        env.bus.emit("y", 2.into()).await;
        env.bus.clone().clear();
        env.bus.clear();
        env.bus.emit("x", 3.into()).await;
        env.bus.emit("y", 3.into()).await;
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |data, _| {
                mark(&trace, serde_json::json!(["new", *data]));
                Ok(None)
            }),
        );
        env.bus.emit("x", 4.into()).await;
        env.verify("channels-clear");
    });
}

#[test]
fn delivery_snapshots_survive_subscription_edits() {
    for action in ["add", "remove", "clear"] {
        run(|env| edit_snapshot(env, action));
    }
}

#[test]
fn single_listener_replacement_affects_the_next_emit() {
    run(|env| async move {
        let bus = env.bus.clone();
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |data, _| {
                mark(&trace, format!("a{data}"));
                bus.clear();
                let trace = trace.clone();
                bus.on(
                    "x",
                    Arc::new(move |data, _| {
                        mark(&trace, format!("b{data}"));
                        Ok(None)
                    }),
                );
                Ok(None)
            }),
        );
        env.bus.emit("x", 1.into()).await;
        env.bus.emit("x", 2.into()).await;
        env.verify("single-snapshot");
    });
}

#[test]
fn nested_emission_uses_its_own_snapshot() {
    run(|env| async move {
        let second = Arc::new(std::sync::Mutex::new(
            None::<maestro_extensions::Subscription>,
        ));
        let state = second.clone();
        let bus = env.bus.clone();
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |data, emitter| {
                mark(&trace, format!("a{data}"));
                if *data == 1 {
                    let handle = state.lock().unwrap().take().unwrap();
                    handle.unsubscribe();
                    let trace = trace.clone();
                    bus.on("x", data_listener(trace, "c"));
                    emitter.emit("x", 2.into());
                }
                Ok(None)
            }),
        );
        let trace = env.trace.clone();
        *second.lock().unwrap() = Some(env.bus.on(
            "x",
            Arc::new(move |data, _| {
                mark(&trace, format!("b{data}"));
                Ok(None)
            }),
        ));
        env.bus.emit("x", 1.into()).await;
        env.verify("nested-snapshot");
    });
}

#[test]
fn ready_tails_start_after_the_emitting_segment() {
    run(ready_tails);
}

#[test]
fn pending_tail_can_resume_its_emitter() {
    run(|env| async move {
        let (release, pending) = tokio::sync::oneshot::channel();
        let (started, start) = tokio::sync::oneshot::channel();
        let (complete, completion) = tokio::sync::oneshot::channel();
        let signals = Arc::new(std::sync::Mutex::new(Some((pending, complete, started))));
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |_, _| {
                mark(&trace, "prefix");
                let trace = trace.clone();
                let (pending, complete, started) = signals.lock().unwrap().take().unwrap();
                Ok(Some(Box::pin(async move {
                    started.send(()).unwrap();
                    pending.await.unwrap();
                    mark(&trace, "tail");
                    complete.send(()).unwrap();
                    Ok(())
                })))
            }),
        );
        env.bus.emit("x", serde_json::Value::Null).await;
        env.mark("emit:return");
        start.await.unwrap();
        release.send(()).unwrap();
        completion.await.unwrap();
        env.mark("emitter:resumed");
        env.settle().await;
        env.verify("pending-tail");
    });
}

#[test]
fn a_pending_tail_does_not_block_other_listeners() {
    run(|env| async move {
        let (release, pending) = tokio::sync::oneshot::channel();
        let pending = Arc::new(std::sync::Mutex::new(Some(pending)));
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |_, _| {
                mark(&trace, "a:prefix");
                let trace = trace.clone();
                let pending = pending.lock().unwrap().take().unwrap();
                Ok(Some(Box::pin(async move {
                    pending.await.unwrap();
                    mark(&trace, "a:tail");
                    Ok(())
                })))
            }),
        );
        let release = Arc::new(std::sync::Mutex::new(Some(release)));
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |_, _| {
                mark(&trace, "b:prefix");
                let trace = trace.clone();
                let release = release.lock().unwrap().take().unwrap();
                Ok(Some(Box::pin(async move {
                    mark(&trace, "b:tail");
                    release.send(()).unwrap();
                    Ok(())
                })))
            }),
        );
        env.bus.emit("x", serde_json::Value::Null).await;
        env.mark("return");
        env.settle().await;
        env.verify("pending-before-ready");
    });
}

#[test]
fn started_tails_survive_removal_and_keep_captured_signals() {
    for action in ["unsubscribe", "clear", "abort"] {
        run(|env| retain_tail(env, action));
    }
}

#[test]
fn listener_errors_are_attributed_without_stopping_delivery() {
    for row in cases("listener_errors_are_attributed_without_stopping_delivery") {
        run(|env| prefix_error(env, row.clone()));
    }
}

#[test]
fn tail_errors_are_reported_after_the_original_emit() {
    run(|env| async move {
        let (release, pending) = tokio::sync::oneshot::channel();
        let pending = Arc::new(std::sync::Mutex::new(Some(pending)));
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |_, _| {
                mark(&trace, "bad:prefix");
                let pending = pending.lock().unwrap().take().unwrap();
                Ok(Some(Box::pin(async move {
                    pending.await.unwrap();
                    Err("tail failure".into())
                })))
            }),
        );
        env.bus.on("x", env.listener("healthy"));
        env.bus.emit("x", serde_json::Value::Null).await;
        env.mark("return");
        assert!(env.errors.lock().unwrap().is_empty());
        env.bus.clear();
        release.send(()).unwrap();
        env.settle().await;
        env.verify("tail-error");
    });
}

#[test]
fn tail_emission_runs_ordinary_listener_prefixes() {
    run(|env| async move {
        let trace = env.trace.clone();
        env.bus.on(
            "outer",
            Arc::new(move |_, emitter| {
                mark(&trace, "outer:prefix");
                let trace = trace.clone();
                let events = emitter.events();
                Ok(Some(Box::pin(async move {
                    events.emit("inner", serde_json::Value::Null).await;
                    mark(&trace, "outer:tail");
                    Ok(())
                })))
            }),
        );
        env.bus
            .for_extension()
            .on("inner", env.listener("inner:prefix"));
        env.bus.emit("outer", serde_json::Value::Null).await;
        env.mark("return");
        env.settle().await;
        env.verify("tail-emit");
    });
}

#[test]
fn reserved_channel_names_have_no_implicit_events() {
    run(|env| async move {
        env.bus.on("newListener", env.listener("implicit:new"));
        let subscription = env.bus.on("x", env.listener("x"));
        env.bus
            .on("removeListener", env.listener("implicit:remove"));
        subscription.unsubscribe();
        env.bus.clear();
        env.verify("magic-metadata");
    });
}

#[test]
fn all_subscribers_are_delivered_without_a_warning_threshold() {
    for row in cases("all_subscribers_are_delivered_without_a_warning_threshold") {
        run(|env| subscriber_count(env, row.clone()));
    }
}

#[test]
fn subscription_and_snapshot_drops_allow_reentrant_cleanup() {
    for action in ["unsubscribe", "clear", "snapshot"] {
        run(|env| reentrant_cleanup(env, action));
    }
}

async fn edit_snapshot(env: support::Env, action: &'static str) {
    env.bus.clear();
    env.trace.lock().unwrap().clear();
    let second = Arc::new(std::sync::Mutex::new(
        None::<maestro_extensions::Subscription>,
    ));
    let state = second.clone();
    let bus = env.bus.clone();
    let trace = env.trace.clone();
    env.bus.on(
        "x",
        Arc::new(move |data, _| {
            mark(&trace, format!("a{data}"));
            if *data == 1 {
                match action {
                    "add" => {
                        let trace = trace.clone();
                        bus.on("x", data_listener(trace, "c"));
                    }
                    "remove" => {
                        let handle = state.lock().unwrap().take().unwrap();
                        handle.unsubscribe();
                    }
                    "clear" => bus.clear(),
                    _ => unreachable!(),
                }
            }
            Ok(None)
        }),
    );
    let trace = env.trace.clone();
    *second.lock().unwrap() = Some(env.bus.on(
        "x",
        Arc::new(move |data, _| {
            mark(&trace, format!("b{data}"));
            Ok(None)
        }),
    ));
    env.bus.emit("x", 1.into()).await;
    env.bus.emit("x", 2.into()).await;
    env.verify(&format!("snapshot-{action}"));
}

async fn ready_tails(env: support::Env) {
    for name in ["a", "b"] {
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |_, _| {
                mark(&trace, format!("{name}:prefix"));
                let trace = trace.clone();
                Ok(Some(Box::pin(async move {
                    mark(&trace, format!("{name}:tail"));
                    Ok(())
                })))
            }),
        );
    }
    env.bus.emit("x", serde_json::Value::Null).await;
    env.mark("emit:return");
    env.settle().await;
    env.verify("ready-tails");
}

async fn retain_tail(env: support::Env, action: &'static str) {
    env.bus.clear();
    env.trace.lock().unwrap().clear();
    let (release, pending) = tokio::sync::oneshot::channel();
    let (started, start) = tokio::sync::oneshot::channel();
    let pending = Arc::new(std::sync::Mutex::new(Some((pending, started))));
    let cancelled = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let signal = cancelled.clone();
    let trace = env.trace.clone();
    let capture = Arc::new(());
    let weak = Arc::downgrade(&capture);
    let handler_capture = capture.clone();
    let subscription = env.bus.on(
        "x",
        Arc::new(move |_, _| {
            mark(&trace, "prefix");
            let trace = trace.clone();
            let signal = signal.clone();
            let capture = handler_capture.clone();
            let (pending, started) = pending.lock().unwrap().take().unwrap();
            Ok(Some(Box::pin(async move {
                started.send(()).unwrap();
                pending.await.unwrap();
                mark(
                    &trace,
                    serde_json::json!(["tail", signal.load(std::sync::atomic::Ordering::SeqCst)]),
                );
                drop(capture);
                Ok(())
            })))
        }),
    );
    drop(capture);
    env.bus.emit("x", serde_json::Value::Null).await;
    start.await.unwrap();
    match action {
        "unsubscribe" => subscription.unsubscribe(),
        "clear" => env.bus.clear(),
        "abort" => {
            cancelled.store(true, std::sync::atomic::Ordering::SeqCst);
            env.bus.clear();
        }
        _ => unreachable!(),
    }
    env.mark(action);
    assert!(weak.upgrade().is_some());
    release.send(()).unwrap();
    env.settle().await;
    assert!(weak.upgrade().is_none());
    env.verify(&format!("retention-{action}"));
}

async fn prefix_error(env: support::Env, row: serde_json::Value) {
    env.bus.clear();
    env.trace.lock().unwrap().clear();
    env.errors.lock().unwrap().clear();
    let diagnostic = row["id"].as_str().unwrap().starts_with("diagnostic");
    let channel = if diagnostic {
        row["input"]["channel"].as_str().unwrap()
    } else {
        "x"
    };
    let cause = if diagnostic {
        row["input"]["cause"].as_str().unwrap()
    } else {
        "prefix failure"
    }
    .to_owned();
    let trace = env.trace.clone();
    env.bus.on(
        channel,
        Arc::new(move |_, _| {
            if !diagnostic {
                mark(&trace, "bad");
            }
            Err(cause.clone().into())
        }),
    );
    if !diagnostic {
        env.bus.on(channel, env.listener("healthy"));
    }
    env.bus.emit(channel, serde_json::Value::Null).await;
    if !diagnostic {
        env.mark("return");
    }
    env.verify(row["id"].as_str().unwrap());
}

async fn subscriber_count(env: support::Env, row: serde_json::Value) {
    env.bus.clear();
    env.trace.lock().unwrap().clear();
    for index in 0..row["input"].as_u64().unwrap() {
        let trace = env.trace.clone();
        env.bus.on(
            "x",
            Arc::new(move |_, _| {
                mark(&trace, index);
                Ok(None)
            }),
        );
    }
    env.bus.emit("x", serde_json::Value::Null).await;
    env.verify(row["id"].as_str().unwrap());
}

struct Cleanup {
    bus: maestro_extensions::EventBusController,
    trace: support::Trace,
}
impl Drop for Cleanup {
    fn drop(&mut self) {
        self.bus.on("cleanup", Arc::new(|_, _| Ok(None)));
        mark(&self.trace, "dropped");
    }
}

async fn reentrant_cleanup(env: support::Env, action: &'static str) {
    env.bus.clear();
    env.trace.lock().unwrap().clear();
    let cleanup = Cleanup {
        bus: env.bus.clone(),
        trace: env.trace.clone(),
    };
    let controller = env.bus.clone();
    let handle = env.bus.on(
        "x",
        Arc::new(move |_, _| {
            let _keep = &cleanup;
            if action == "snapshot" {
                controller.clear();
            }
            Ok(None)
        }),
    );
    match action {
        "unsubscribe" => handle.unsubscribe(),
        "clear" => env.bus.clear(),
        "snapshot" => env.bus.emit("x", serde_json::Value::Null).await,
        _ => unreachable!(),
    }
    assert_eq!(*env.trace.lock().unwrap(), [serde_json::json!("dropped")]);
    env.bus.clear();
}

fn data_listener(trace: support::Trace, prefix: &'static str) -> maestro_extensions::EventListener {
    Arc::new(move |data, _| {
        mark(&trace, format!("{prefix}{data}"));
        Ok(None)
    })
}

fn cases(test: &str) -> Vec<serde_json::Value> {
    let all: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/event_traces.json")).unwrap();
    let selected: Vec<_> = all
        .into_iter()
        .filter(|case| case["test"] == test)
        .collect();
    assert!(!selected.is_empty(), "no corpus cases for {test}");
    selected
}
