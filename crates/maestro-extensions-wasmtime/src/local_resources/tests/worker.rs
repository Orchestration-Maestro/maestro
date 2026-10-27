//! Native pump retirement owns Store thread termination without blocking its executor.
use super::Pump;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Notify;

/// Attaches an observed native worker to the same owner as the production Store thread.
fn own_worker(pump: &Pump, work: impl FnOnce(Arc<Notify>) + Send + 'static) -> Arc<AtomicBool> {
    let shutdown = Arc::new(Notify::new());
    let weak = Arc::downgrade(&shutdown);
    let finished = Arc::new(AtomicBool::new(false));
    let observed = finished.clone();
    let worker = std::thread::spawn(move || {
        work(shutdown);
        observed.store(true, Ordering::SeqCst);
    });
    pump.own_worker(worker, weak);
    finished
}

/// Waits for the native owner using the Store worker's existing runtime facility.
fn wait_for_shutdown(shutdown: &Notify) {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(shutdown.notified());
}

#[test]
fn pump_retirement_keeps_the_live_runtime_runnable_until_the_worker_can_exit() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let finished = runtime.block_on(async {
        let (pump, admission) = Pump::new(crate::tests::support::widgets());
        let (release, released) = std::sync::mpsc::channel();
        let (entered, observed) = tokio::sync::oneshot::channel();
        let finished = own_worker(&pump, move |shutdown| {
            wait_for_shutdown(&shutdown);
            entered.send(()).unwrap();
            released.recv().unwrap();
        });
        drop(pump);
        observed.await.unwrap();
        assert!(!finished.load(Ordering::SeqCst));
        release.send(()).unwrap();
        drop(admission);
        finished
    });
    drop(runtime);
    assert!(finished.load(Ordering::SeqCst));
}

#[test]
fn local_set_retirement_joins_the_worker_after_its_runtime_has_shut_down() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    let finished = local.block_on(&runtime, async {
        let (pump, admission) = Pump::new(crate::tests::support::widgets());
        let finished = own_worker(&pump, |shutdown| wait_for_shutdown(&shutdown));
        let (started, observed) = tokio::sync::oneshot::channel();
        tokio::task::spawn_local(async move {
            let owners = (pump, admission);
            started.send(()).unwrap();
            std::future::pending::<()>().await;
            drop(owners);
        });
        observed.await.unwrap();
        finished
    });
    drop(runtime);
    drop(local);
    assert!(finished.load(Ordering::SeqCst));
}

#[test]
fn pump_retirement_joins_the_worker_when_runtime_shutdown_discards_the_offloaded_join() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let handle = runtime.handle().clone();
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let finished = own_worker(&pump, |shutdown| wait_for_shutdown(&shutdown));
    drop(runtime);
    let context = handle.enter();
    drop(pump);
    assert!(finished.load(Ordering::SeqCst));
    drop(context);
    drop(admission);
}

#[test]
fn pump_retirement_ends_a_faulted_store_while_native_senders_remain() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let (calls, receiver) = tokio::sync::mpsc::unbounded_channel();
    let finished = own_worker(&pump, move |shutdown| {
        let mut incoming = crate::lifecycle::Incoming::new(receiver, shutdown);
        tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap()
            .block_on(crate::lifecycle::faulted(&mut incoming));
    });
    drop(pump);
    assert!(finished.load(Ordering::SeqCst));
    drop(calls);
    drop(admission);
}

#[test]
fn pump_retirement_disconnects_queued_native_replies_before_joining_the_worker() {
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let (reply, response) = std::sync::mpsc::channel();
    registry
        .requests
        .upgrade()
        .unwrap()
        .send(crate::local_resources::Message::Signal(
            0,
            crate::local_resources::signals::Request::Aborted(reply),
        ))
        .unwrap();
    let finished = own_worker(&pump, move |_shutdown| {
        assert!(response.recv().is_err());
    });
    drop(pump);
    assert!(finished.load(Ordering::SeqCst));
    drop(admission);
}
