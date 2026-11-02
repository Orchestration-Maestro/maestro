//! A real Store fault retains native table ownership until the last admission ends.
use super::*;
use crate::tests::{api, session};
use maestro_extensions::{
    ExtensionCommandContextActions, ExtensionRunner, create_extension_runtime,
};
use std::sync::{
    RwLock,
    atomic::{AtomicUsize, Ordering},
};
use wasmtime_wasi::WasiView;

/// Observes actual Store table destruction, not merely a native callback's release.
struct StoreDrop {
    /// Exactly-once observation shared with the native test owner.
    count: Arc<AtomicUsize>,
    /// Completion signal avoids timing-based teardown assertions.
    dropped: Option<tokio::sync::oneshot::Sender<()>>,
}
impl Drop for StoreDrop {
    fn drop(&mut self) {
        self.count.fetch_add(1, Ordering::SeqCst);
        let _ = self.dropped.take().unwrap().send(());
    }
}

#[test]
fn faulted_store_drops_once_only_after_the_last_admission_ends() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, observe_faulted_store(false));
}

/// Enters the production worker with an observed table resource and an actual authored trap.
async fn observe_faulted_store(release_trap: bool) {
    let native = create_extension_runtime::<Local>();
    let (owner, mut api) = api::api(native.clone());
    let bus = maestro_extensions::create_event_bus_for::<Local>(Rc::new(drop), Rc::new(|_| {}));
    api.events = bus.for_extension();
    let (pump, admission) = Pump::new(crate::tests::support::widgets());
    let endpoint = pump.registry().api(api).unwrap();
    let (calls, incoming) = mpsc::unbounded_channel();
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let retirement = Arc::downgrade(&shutdown);
    let mut incoming = lifecycle::Incoming::new(incoming, shutdown);
    let replies = Replies::default();
    let callbacks = Callbacks::new(calls.downgrade(), replies.clone());
    let weak = calls.downgrade();
    let retained = lifecycle::StoreEndpoint::new(calls, admission, replies.clone());
    let path = std::env::var("MAESTRO_CONTEXT_COMPONENT").unwrap();
    let version = installation();
    let mut state = lifecycle::State::new(endpoint, callbacks, &path, version, None).unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let (dropped, observed) = tokio::sync::oneshot::channel();
    state
        .ctx()
        .table
        .push(StoreDrop {
            count: count.clone(),
            dropped: Some(dropped),
        })
        .unwrap();
    let worker =
        std::thread::spawn(move || super::worker(&path, state, &mut incoming, weak, &replies));
    pump.own_worker(worker, retirement);
    let pump = tokio::task::spawn_local(async move {
        pump.serve().await;
    });
    retained.submit(Call::Start).await.unwrap();
    let runner = ExtensionRunner::<Local>::new(
        Rc::new(RwLock::new(vec![owner.clone()])),
        native,
        session::runner_session("/observed-fault").0,
        Rc::new(|_| {}),
        maestro_theme::LiveTheme::default(),
    );
    if release_trap {
        trap_during_release(&runner, &owner, &retained.replies).await;
    } else {
        trap_and_release(&runner).await;
    }
    assert_eq!(count.load(Ordering::SeqCst), 0);
    drop(runner);
    drop(owner);
    bus.clear();
    let fault = retained.submit(Call::Start).await.unwrap_err();
    assert!(fault.contains("wasm trap"), "{fault}");
    assert_eq!(count.load(Ordering::SeqCst), 0);
    drop(retained);
    observed.await.unwrap();
    pump.await.unwrap();
    assert_eq!(count.load(Ordering::SeqCst), 1);
}

/// Traps through the actual retained command rather than a synthetic failed reply.
async fn trap_and_release(runner: &ExtensionRunner<Local>) {
    super::super::tests::support::bind_prompt(runner, "fault observer");
    runner.bind_command_context(Some(ExtensionCommandContextActions::new(
        maestro_extensions::CommandContextOperations {
            wait_for_idle: Rc::new(|| Box::pin(async { Ok(()) })),
            ..crate::context_actions::silent_command_operations()
        },
    )));
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let fault = command("trap".into(), runner.create_command_context())
        .await
        .unwrap_err()
        .to_string();
    assert!(fault.contains("wasm trap"), "{fault}");
}

#[test]
fn trapping_callback_release_drops_store_and_pump_after_external_admissions_end() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, observe_faulted_store(true));
}

/// Final callback release itself traps, without any native invocation reply to complete it.
async fn trap_during_release(
    runner: &ExtensionRunner<Local>,
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
    replies: &Replies,
) {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    let armed = command("arm-release-trap".into(), runner.create_command_context()).await;
    assert!(
        armed
            .unwrap_err()
            .to_string()
            .starts_with("command:arm-release-trap|")
    );
    let (_token, fault) = replies.reply::<()>().unwrap();
    owner
        .write()
        .unwrap()
        .commands
        .shift_remove("bridge-observe");
    drop(command);
    assert!(fault.await.unwrap().unwrap_err().contains("wasm trap"));
}

/// Fixed installation facts for the observed Store lifecycle.
fn installation() -> Installation {
    Installation {
        version: "installed-component-host".into(),
        agent_dir: "/controlled-agent".into(),
    }
}
