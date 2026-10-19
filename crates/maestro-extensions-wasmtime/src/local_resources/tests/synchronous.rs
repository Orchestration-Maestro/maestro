//! Native synchronous callbacks service nested requests through their original pump.
use crate::read_imports::ReadEndpoint;
use crate::{
    context_reads::{Owner, Request},
    local_resources::{Pump, synchronous::Completion},
    tests::runner,
};
use maestro_extensions::{AgentToolUpdateCallback, Local};
use std::{rc::Rc, sync::mpsc};

#[test]
fn synchronous_native_callbacks_service_nested_original_context_reads() {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(async {
            let runner = runner("/synchronous-original");
            let (pump, admission) = Pump::new(crate::tests::support::credential_operations());
            let registry = pump.registry();
            let context = registry
                .context(Owner::Base(runner.create_context()))
                .unwrap();
            let (first, first_calls) = mpsc::channel::<Completion>();
            let (second, second_calls) = mpsc::channel::<Completion>();
            let first_registry = registry.clone();
            let first_callback: AgentToolUpdateCallback<serde_json::Value, Local> =
                Rc::new(move |_| {
                    first_registry
                        .invoke_sync(|completion| {
                            first.send(completion).map_err(|error| error.to_string())
                        })
                        .unwrap();
                });
            let second_registry = registry.clone();
            let second_callback: AgentToolUpdateCallback<serde_json::Value, Local> =
                Rc::new(move |_| {
                    assert_eq!(
                        second_registry
                            .invoke_sync(|completion| second
                                .send(completion)
                                .map_err(|error| error.to_string()))
                            .unwrap_err(),
                        "nested guest failure"
                    );
                });
            let first_update = registry.tool_update(first_callback).unwrap();
            let second_update = registry.tool_update(second_callback).unwrap();
            let second_context = context.clone();
            let nested = std::thread::spawn(move || {
                let completion = second_calls.recv().unwrap();
                assert_eq!(
                    second_context.read(Request::Cwd).unwrap(),
                    "/synchronous-original"
                );
                completion.complete(Err("nested guest failure".into()));
            });
            let outer = std::thread::spawn(move || {
                let completion = first_calls.recv().unwrap();
                assert_eq!(context.read(Request::Cwd).unwrap(), "/synchronous-original");
                second_update.update(r#"{"content":[]}"#.into()).unwrap();
                completion.complete(Ok(()));
            });
            let caller = std::thread::spawn(move || {
                first_update.update(r#"{"content":[]}"#.into()).unwrap();
            });
            drop(registry);
            drop(admission);
            pump.serve().await;
            caller.join().unwrap();
            outer.join().unwrap();
            nested.join().unwrap();
        });
}

#[test]
fn abandoned_synchronous_reply_is_catchable_and_does_not_close_the_pump() {
    let (pump, admission) = Pump::new(crate::tests::support::credential_operations());
    let registry = pump.registry();
    assert_eq!(
        registry
            .invoke_sync(|completion| {
                drop(completion);
                Ok(())
            })
            .unwrap_err(),
        "extension guest reply ended"
    );
    registry
        .invoke_sync(|completion| {
            completion.complete(Ok(()));
            Ok(())
        })
        .unwrap();
    drop(pump);
    drop(admission);
    assert_eq!(
        registry.invoke_sync(|_| Ok(())).unwrap_err(),
        "extension resource owner ended"
    );
}

/// Observes whether the original asynchronous owner is made runnable again.
#[derive(Default)]
struct ExecutorWake(std::sync::atomic::AtomicUsize);
impl std::task::Wake for ExecutorWake {
    fn wake(self: std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
    fn wake_by_ref(self: &std::sync::Arc<Self>) {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

#[test]
fn synchronous_wait_returns_receiver_wakeups_to_the_suspended_async_owner() {
    use std::{
        future::Future,
        sync::{Arc, atomic::Ordering},
        task::{Context, Waker},
    };
    let (mut pump, admission) = Pump::new(crate::tests::support::credential_operations());
    let registry = pump.registry();
    let executor = Arc::new(ExecutorWake::default());
    let observed = executor.clone();
    let update = registry
        .tool_update(Rc::new(move |_| {
            observed.0.store(0, Ordering::SeqCst);
        }))
        .unwrap();
    let waker = Waker::from(executor.clone());
    let mut pending_owner = Box::pin(pump.answer_next());
    assert!(Future::poll(pending_owner.as_mut(), &mut Context::from_waker(&waker)).is_pending());
    let (calls, incoming) = mpsc::channel::<Completion>();
    let guest = std::thread::spawn(move || {
        let completion = incoming.recv().unwrap();
        update.update(r#"{"content":[]}"#.into()).unwrap();
        completion.complete(Ok(()));
    });
    registry
        .invoke_sync(|completion| calls.send(completion).map_err(|error| error.to_string()))
        .unwrap();
    guest.join().unwrap();
    assert!(
        executor.0.load(Ordering::SeqCst) > 0,
        "async owner was not scheduled after synchronous receiver ownership ended"
    );
    drop((pending_owner, registry, admission));
}
