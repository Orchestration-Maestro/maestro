#![cfg(test)]
#![cfg(not(target_arch = "wasm32"))]
mod support;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use support::{mark, run};

#[test]
fn synchronous_callback_return_releases_foreign_listeners() {
    run(|env| async move {
        let owner = env.bus.for_extension();
        let foreign = env.bus.for_extension();
        foreign.on("x", env.listener("foreign"));
        owner.on("x", env.listener("same"));
        owner.invoke_callback(|emitter| {
            env.mark("body:start");
            emitter.emit("x", Value::Null);
            env.mark("body:end");
        });
        env.mark("release");
        env.verify("callback-return");
    });
}

#[test]
fn same_extension_delivery_is_immediately_nested() {
    run(|env| async move {
        env.bus.on("inner", env.listener("same"));
        env.bus.clone().invoke_callback(|emitter| {
            env.mark("start");
            emitter.emit("inner", Value::Null);
            env.mark("end");
        });
        env.verify("same-nested");
        env.bus.clear();
        env.trace.lock().unwrap().clear();
        nested_tails(&env, "same-ready").await;
    });
}

async fn nested_tails(env: &support::Env, mode: &str) {
    let a = env.bus.for_extension();
    let b = if mode == "same-ready" {
        a.clone()
    } else {
        env.bus.for_extension()
    };
    let pending = mode == "foreign-pending";
    let (back_send, back_receive) = tokio::sync::oneshot::channel();
    let (b_send, b_receive) = tokio::sync::oneshot::channel();
    let back_receive = Arc::new(Mutex::new(Some(back_receive)));
    let b_receive = Arc::new(Mutex::new(Some(b_receive)));
    let releases = Arc::new(Mutex::new(Some((back_send, b_send))));
    let trace = env.trace.clone();
    a.on(
        "back",
        Arc::new(move |_, _| {
            mark(&trace, "a:back");
            let trace = trace.clone();
            let signal = back_receive.lock().unwrap().take().unwrap();
            Ok(Some(Box::pin(async move {
                if pending {
                    signal.await.unwrap();
                }
                mark(&trace, "a:back:tail");
                Ok(())
            })))
        }),
    );
    let trace = env.trace.clone();
    b.on(
        "inner",
        Arc::new(move |_, emitter| {
            mark(&trace, "b:start");
            emitter.emit("back", Value::Null);
            mark(&trace, "b:end");
            let trace = trace.clone();
            let signal = b_receive.lock().unwrap().take().unwrap();
            Ok(Some(Box::pin(async move {
                if pending {
                    signal.await.unwrap();
                }
                mark(&trace, "b:tail");
                Ok(())
            })))
        }),
    );
    install_outer_tail(env, &a, releases, pending);
    env.bus.emit("outer", Value::Null).await;
    env.mark("return");
    env.settle().await;
    env.verify(&format!("nested-tails:{mode}"));
    env.bus.clear();
}

#[test]
fn foreign_reentry_drains_at_each_callback_return() {
    run(|env| async move {
        let a = env.bus.for_extension();
        let b = env.bus.for_extension();
        let trace = env.trace.clone();
        a.on(
            "outer",
            Arc::new(move |_, emitter| {
                mark(&trace, "a:start");
                emitter.emit("inner", Value::Null);
                mark(&trace, "a:end");
                Ok(None)
            }),
        );
        let trace = env.trace.clone();
        b.on(
            "inner",
            Arc::new(move |_, emitter| {
                mark(&trace, "b:start");
                emitter.emit("back", Value::Null);
                mark(&trace, "b:end");
                Ok(None)
            }),
        );
        a.on("back", env.listener("a:back"));
        env.bus.emit("outer", Value::Null).await;
        env.mark("return");
        env.verify("reentry");
        env.bus.clear();
        env.trace.lock().unwrap().clear();
        nested_tails(&env, "foreign-ready").await;
        env.trace.lock().unwrap().clear();
        nested_tails(&env, "foreign-pending").await;
    });
}

#[test]
fn queued_foreign_delivery_keeps_the_original_snapshot() {
    run(|env| async move {
        let owner = env.bus.for_extension();
        let foreign = env.bus.for_extension();
        let trace = env.trace.clone();
        let subscription = foreign.on(
            "x",
            Arc::new(move |data, _| {
                mark(&trace, format!("foreign:{data}"));
                Ok(None)
            }),
        );
        owner.invoke_callback(|emitter| {
            env.mark("start");
            emitter.emit("x", 1.into());
            subscription.unsubscribe();
            env.bus.clear();
            let trace = env.trace.clone();
            foreign.on(
                "x",
                Arc::new(move |data, _| {
                    mark(&trace, format!("new:{data}"));
                    Ok(None)
                }),
            );
            env.mark("end");
        });
        owner.emit("x", 2.into()).await;
        env.verify("deferred-snapshot");
    });
}

#[test]
fn foreign_delivery_precedes_the_following_release_callback() {
    run(|env| async move {
        let owner = env.bus.for_extension();
        env.bus.for_extension().on("x", env.listener("foreign"));
        owner.invoke_callback(|emitter| {
            env.mark("dispose:start");
            emitter.emit("x", Value::Null);
            env.mark("dispose:end");
        });
        owner.invoke_callback(|_| env.mark("release"));
        owner.invoke_callback(|emitter| {
            env.mark("factory:start");
            emitter.emit("x", Value::Null);
            env.mark("factory:end");
        });
        env.mark("installed");
        env.verify("each-return");
    });
}

#[test]
fn nested_callbacks_keep_their_own_foreign_queues() {
    run(|env| async move {
        let owner = env.bus.for_extension();
        let trace = env.trace.clone();
        env.bus.for_extension().on(
            "x",
            Arc::new(move |data, _| {
                mark(&trace, format!("foreign:{}", data.as_str().unwrap()));
                Ok(None)
            }),
        );
        owner.invoke_callback(|outer| {
            env.mark("outer:start");
            outer.emit("x", "outer".into());
            owner.invoke_callback(|inner| {
                env.mark("inner:start");
                inner.emit("x", "inner".into());
                env.mark("inner:end");
            });
            env.mark("outer:end");
        });
        env.mark("return");
        env.verify("nested-scopes");
    });
}

#[test]
fn callback_errors_do_not_discard_queued_delivery() {
    run(|env| async move {
        env.bus.on(
            "outer",
            Arc::new(move |_, emitter| {
                emitter.emit("x", Value::Null);
                Err("prefix failure".into())
            }),
        );
        env.bus.for_extension().on("x", env.listener("foreign"));
        env.bus.emit("outer", Value::Null).await;
        env.verify("callback-error");
        let result: Result<(), &str> = env.bus.invoke_callback(|emitter| {
            emitter.emit("x", Value::Null);
            Err("factory failure")
        });
        assert_eq!(result, Err("factory failure"));
        assert_eq!(
            *env.trace.lock().unwrap(),
            [json!("foreign"), json!("foreign")]
        );
    });
}

#[test]
fn overlapping_origins_finish_in_non_stack_order() {
    run(|env| async move {
        let a = env.bus.for_extension();
        let b = env.bus.for_extension();
        let (a_release, a_pending) = tokio::sync::oneshot::channel();
        let (b_release, b_pending) = tokio::sync::oneshot::channel();
        let (a_complete, a_completion) = tokio::sync::oneshot::channel();
        let (b_complete, b_completion) = tokio::sync::oneshot::channel();
        let a_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let b_signal = Arc::new(std::sync::atomic::AtomicBool::new(false));
        for (name, owner, foreign, pending, complete, signal) in [
            ("a", &a, &b, a_pending, a_complete, a_signal),
            ("b", &b, &a, b_pending, b_complete, b_signal.clone()),
        ] {
            install_origin(
                &env,
                name,
                owner,
                foreign,
                OriginSignals {
                    pending,
                    complete,
                    signal,
                },
            );
        }
        let (a_started, a_start) = tokio::sync::oneshot::channel();
        let (b_started, b_start) = tokio::sync::oneshot::channel();
        let trace = env.trace.clone();
        let a_root = tokio::spawn(async move {
            a.emit("a", Value::Null).await;
            a_started.send(()).unwrap();
            a_completion.await.unwrap();
            mark(&trace, "a:done");
        });
        a_start.await.unwrap();
        let trace = env.trace.clone();
        let b_root = tokio::spawn(async move {
            b.emit("b", Value::Null).await;
            b_started.send(()).unwrap();
            b_completion.await.unwrap();
            mark(&trace, "b:done");
        });
        b_start.await.unwrap();
        b_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        a_release.send(()).unwrap();
        a_root.await.unwrap();
        b_release.send(()).unwrap();
        b_root.await.unwrap();
        env.settle().await;
        env.verify("overlap");
    });
}

#[test]
fn failed_instance_listeners_stay_registered() {
    run(|env| async move {
        let failed = env.bus.for_extension();
        let healthy = env.bus.for_extension();
        let entered = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let poisoned = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let count = entered.clone();
        let trace = env.trace.clone();
        failed.on(
            "x",
            Arc::new(move |_, _| {
                mark(&trace, "attempt");
                if !poisoned.swap(true, std::sync::atomic::Ordering::SeqCst) {
                    count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                Err("instance a trapped".into())
            }),
        );
        healthy.on("x", env.listener("healthy"));
        env.bus.emit("x", Value::Null).await;
        env.bus.emit("x", Value::Null).await;
        env.verify("repeat-error");
        assert_eq!(entered.load(std::sync::atomic::Ordering::SeqCst), 1);
        let attempts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = attempts.clone();
        failed.on(
            "recover",
            Arc::new(move |_, _| {
                if count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                    Err("recoverable".into())
                } else {
                    Ok(None)
                }
            }),
        );
        env.errors.lock().unwrap().clear();
        env.bus.emit("recover", Value::Null).await;
        env.bus.emit("recover", Value::Null).await;
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert_eq!(
            *env.errors.lock().unwrap(),
            ["Event handler error (recover): recoverable\n"]
        );
    });
}

type Releases = Arc<
    Mutex<
        Option<(
            tokio::sync::oneshot::Sender<()>,
            tokio::sync::oneshot::Sender<()>,
        )>,
    >,
>;
fn install_outer_tail(
    env: &support::Env,
    a: &maestro_extensions::EventBus,
    releases: Releases,
    pending: bool,
) {
    let trace = env.trace.clone();
    a.on(
        "outer",
        Arc::new(move |_, emitter| {
            mark(&trace, "a:start");
            emitter.emit("inner", Value::Null);
            mark(&trace, "a:end");
            let trace = trace.clone();
            let (back, b) = releases.lock().unwrap().take().unwrap();
            Ok(Some(Box::pin(async move {
                mark(&trace, "a:tail");
                if pending {
                    back.send(()).unwrap();
                    b.send(()).unwrap();
                }
                Ok(())
            })))
        }),
    );
}

struct OriginSignals {
    pending: tokio::sync::oneshot::Receiver<()>,
    complete: tokio::sync::oneshot::Sender<()>,
    signal: Arc<std::sync::atomic::AtomicBool>,
}
fn install_origin(
    env: &support::Env,
    name: &'static str,
    owner: &maestro_extensions::EventBus,
    foreign: &maestro_extensions::EventBus,
    signals: OriginSignals,
) {
    let OriginSignals {
        pending,
        complete,
        signal,
    } = signals;
    let identity_trace = Arc::new(Mutex::new(Vec::<Value>::new()));
    let channel = format!("identity:{name}");
    let trace = identity_trace.clone();
    foreign.on(
        &channel,
        Arc::new(move |_, _| {
            mark(&trace, "foreign");
            Ok(None)
        }),
    );
    let trace = identity_trace.clone();
    owner.on(
        &channel,
        Arc::new(move |_, _| {
            mark(&trace, "own");
            Ok(None)
        }),
    );
    let signals = Arc::new(Mutex::new(Some((pending, complete))));
    let trace = env.trace.clone();
    owner.on(
        name,
        Arc::new(move |_, emitter| {
            mark(&trace, format!("{name}:prefix"));
            let events = emitter.events();
            let trace = trace.clone();
            let signal = signal.clone();
            let identity_trace = identity_trace.clone();
            let channel = channel.clone();
            let (pending, complete) = signals.lock().unwrap().take().unwrap();
            Ok(Some(Box::pin(async move {
                pending.await.unwrap();
                events.invoke_callback(|emitter| {
                    emitter.emit(&channel, Value::Null);
                    mark(&identity_trace, "marker");
                });
                assert_eq!(
                    *identity_trace.lock().unwrap(),
                    [json!("own"), json!("marker"), json!("foreign")]
                );
                mark(
                    &trace,
                    json!([
                        format!("{name}:tail"),
                        signal.load(std::sync::atomic::Ordering::SeqCst)
                    ]),
                );
                complete.send(()).unwrap();
                Ok(())
            })))
        }),
    );
}
