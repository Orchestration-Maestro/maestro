//! Component abort listeners observe the original native signal on its owning thread.
#![cfg(test)]
use maestro_extensions::{
    Cancellation, EventBus, ExtensionContextActions, ExtensionExecution, ExtensionRunner, Local,
    create_event_bus_for, types::ProviderActions,
};
use std::{cell::RefCell, rc::Rc, thread::ThreadId};
use tokio::sync::mpsc;

/// Each observed listener label with the thread it ran on.
pub(super) type Observations = Rc<RefCell<Vec<(String, ThreadId)>>>;

/// Supplies `signal` as the context signal of every later command.
pub(super) fn bind(runner: &ExtensionRunner<Local>, signal: &Cancellation) {
    let signal = signal.clone();
    runner
        .bind_core(
            Local::unbound_actions(),
            ExtensionContextActions {
                get_signal: Rc::new(move || Ok(Some(signal.clone()))),
                ..Default::default()
            },
            ProviderActions::default(),
        )
        .unwrap();
}

/// Runs one signal request of the bridge command and returns its reported outcome.
pub(super) async fn signal_command(runner: &ExtensionRunner<Local>, request: &str) -> String {
    let command = runner
        .get_command("bridge-observe")
        .unwrap()
        .command
        .handler;
    command(format!("signal {request}"), runner.create_command_context())
        .await
        .unwrap_err()
        .to_string()
}

/// Records every observed listener and announces each one on `seen`.
pub(super) fn observe(events: &EventBus<Local>) -> (Observations, mpsc::UnboundedReceiver<()>) {
    let observations = Observations::default();
    let (announce, seen) = mpsc::unbounded_channel();
    let record = observations.clone();
    drop(events.on(
        "abort-observed",
        Rc::new(move |data, _| {
            let label = data.as_str().unwrap().to_owned();
            record
                .borrow_mut()
                .push((label, std::thread::current().id()));
            announce.send(()).unwrap();
            Ok(None)
        }),
    ));
    (observations, seen)
}

/// Completes once every clone of `signal` is gone: a kept native observer of it is released
/// with its last clone on this thread.
pub(super) fn signal_release(signal: &Cancellation) -> tokio::sync::oneshot::Receiver<()> {
    let (released, release) = tokio::sync::oneshot::channel();
    let released = super::lifetimes::Released(Some(released));
    drop(signal.subscribe(Rc::new(move || {
        let _retained = &released;
    })));
    release
}

/// How many of the observed signals some owner still retains.
pub(super) fn retained(releases: &mut [tokio::sync::oneshot::Receiver<()>]) -> usize {
    releases
        .iter_mut()
        .map(tokio::sync::oneshot::Receiver::try_recv)
        .filter(Result::is_err)
        .count()
}

/// Requires the signal to stay shareable across threads.
fn send_sync<T: Send + Sync>(_: &T) {}

/// The labels observed so far, each on the calling thread.
pub(super) fn labels(observations: &Observations) -> Vec<String> {
    let here = std::thread::current().id();
    observations
        .borrow()
        .iter()
        .map(|(label, thread)| {
            assert_eq!(*thread, here, "{label} ran on another thread");
            label.clone()
        })
        .collect()
}

#[test]
fn native_abort_observers_keep_signal_send_and_owner_affinity() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, events) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        let (observations, mut seen) = observe(&events);
        let signal = Cancellation::new();
        send_sync(&signal);
        bind(&runner, &signal);
        assert_eq!(signal_command(&runner, "subscribe a").await, "subscribed a");
        assert_eq!(signal_command(&runner, "subscribe b").await, "subscribed b");

        let foreign = signal.clone();
        std::thread::spawn(move || foreign.abort()).join().unwrap();
        assert!(
            labels(&observations).is_empty(),
            "the foreign thread ran a listener"
        );
        seen.recv().await.unwrap();
        seen.recv().await.unwrap();
        assert_eq!(labels(&observations), ["a", "b"]);

        let local = Cancellation::new();
        bind(&runner, &local);
        assert_eq!(signal_command(&runner, "subscribe c").await, "subscribed c");
        local.abort();
        assert_eq!(labels(&observations), ["a", "b", "c"]);
        signal.abort();
        local.abort();
        assert_eq!(labels(&observations), ["a", "b", "c"]);
    });
}

#[test]
fn dropped_signal_remover_keeps_foreign_abort_observation() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, events) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        let (observations, mut seen) = observe(&events);
        let local = Cancellation::new();
        bind(&runner, &local);
        signal_command(&runner, "subscribe local").await;
        assert_eq!(signal_command(&runner, "drop local").await, "dropped local");
        local.abort();
        assert_eq!(labels(&observations), ["local"]);
        seen.try_recv().unwrap();

        let foreign = Cancellation::new();
        bind(&runner, &foreign);
        signal_command(&runner, "subscribe foreign").await;
        assert_eq!(
            signal_command(&runner, "drop foreign").await,
            "dropped foreign"
        );
        let barrier = Cancellation::new();
        bind(&runner, &barrier);
        signal_command(&runner, "subscribe barrier").await;
        std::thread::spawn(move || {
            foreign.abort();
            barrier.abort();
        })
        .join()
        .unwrap();
        while !labels(&observations).iter().any(|label| label == "barrier") {
            seen.recv().await.unwrap();
        }
        let mut observed = labels(&observations);
        observed.sort();
        assert_eq!(observed, ["barrier", "foreign", "local"]);
    });
}

#[test]
fn removed_abort_observers_release_their_signals() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(|_| panic!("no errors")),
        );
        let (_owner, runner, _events) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        let mut releases = Vec::new();
        for label in ["first", "second", "third"] {
            let signal = Cancellation::new();
            releases.push(signal_release(&signal));
            bind(&runner, &signal);
            signal_command(&runner, &format!("subscribe {label}")).await;
            assert_eq!(
                signal_command(&runner, &format!("unsubscribe {label}")).await,
                format!("unsubscribed {label}")
            );
        }
        bind(&runner, &Cancellation::new());
        signal_command(&runner, "barrier").await;
        assert_eq!(
            retained(&mut releases),
            0,
            "removed observers kept their signals"
        );
    });
}

/// Whether the component registered the flag naming a released listener capture.
pub(super) fn released(
    owner: &maestro_extensions::types::ExtensionHandle<Local>,
    label: &str,
) -> bool {
    owner
        .read()
        .unwrap()
        .flags
        .contains_key(&format!("released-{label}"))
}

#[test]
fn native_abort_observers_release_at_the_last_consumer() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let errors = Rc::new(RefCell::new(Vec::new()));
        let (reporter, teardown) = super::lifetimes::observed_reporter(&errors);
        let bus = create_event_bus_for::<Local>(Rc::new(|_| panic!("no tails")), reporter);
        let (owner, runner, events) =
            super::retained_components::loaded_on(bus.for_extension()).await;
        let (observations, _seen) = observe(&events);
        let signal = Cancellation::new();
        bind(&runner, &signal);
        signal_command(&runner, "subscribe a").await;
        signal_command(&runner, "subscribe b").await;
        assert_eq!(
            signal_command(&runner, "unsubscribe a").await,
            "unsubscribed a"
        );
        signal_command(&runner, "barrier").await;
        assert!(released(&owner, "a"));
        assert!(!released(&owner, "b"));
        signal.abort();
        assert_eq!(labels(&observations), ["b"]);
        signal_command(&runner, "barrier").await;
        assert!(released(&owner, "b"));

        let retained = Cancellation::new();
        bind(&runner, &retained);
        signal_command(&runner, "subscribe kept").await;
        assert!(!released(&owner, "kept"));
        drop((owner, runner, events));
        bus.clear();
        drop(bus);
        teardown.await.unwrap();
        assert!(errors.borrow().is_empty());
        assert!(!retained.is_aborted());
    });
}
