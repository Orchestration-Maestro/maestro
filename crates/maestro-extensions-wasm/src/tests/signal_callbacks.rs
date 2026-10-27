//! Cancellation listeners on host signals through the controlled adapter.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use super::bus_callbacks::{Reenter, captured_api};
use super::bus_host::Emitter;
use super::controlled::{Controlled, Flag};
use super::observed::gate;
use crate::component_adapter::{Exports, events, signal};
use crate::{AbortListener, AbortSignal, ExecOptions, ExtensionResult};
use serde_json::Value;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::task::{Context, Waker};

/// A host signal reading `flag`.
fn host_signal(host: &Controlled, flag: &Flag) -> AbortSignal {
    signal(host, host.signal(flag))
}

/// A caller-defined cancellation port without a retained component resource.
struct Authored(AbortSignal);
impl crate::SignalPort for Authored {
    fn aborted(&self) -> bool {
        self.0.aborted()
    }
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<crate::Subscription> {
        self.0.subscribe(listener)
    }
}

/// Supplies an author-defined port so forwarding must retain a temporary mirror.
pub(super) fn authored(signal: AbortSignal) -> AbortSignal {
    AbortSignal::new(Rc::new(Authored(signal)))
}

/// A listener that counts its runs.
fn counting(runs: &Rc<Cell<usize>>) -> AbortListener {
    let runs = Rc::clone(runs);
    Rc::new(move |_| {
        runs.set(runs.get() + 1);
        Ok(())
    })
}

/// The identity of the latest registered listener.
fn latest(host: &Controlled) -> u32 {
    host.observed()
        .bus
        .listeners
        .last()
        .map(|(_, id)| *id)
        .unwrap()
}

#[test]
fn signal_host_notification_runs_once() -> ExtensionResult<()> {
    let host = Controlled::default();
    let flag = Flag::default();
    let signal = host_signal(&host, &flag);
    let runs = Rc::new(Cell::new(0));
    let own = Rc::new(Cell::new(0));
    let repeated = Rc::new(RefCell::new(None));
    let listener: AbortListener = {
        let (host, runs, own, repeated) = (
            host.clone(),
            Rc::clone(&runs),
            Rc::clone(&own),
            Rc::clone(&repeated),
        );
        Rc::new(move |_| {
            runs.set(runs.get() + 1);
            let emitter = Emitter { key: 0 };
            *repeated.borrow_mut() =
                Some(Exports::new(host.clone()).invoke_abort(own.get(), &emitter));
            Ok(())
        })
    };
    let _subscription = signal.subscribe(listener)?;
    own.set(latest(&host));

    assert_eq!(host.abort(&flag), [Ok(())]);
    assert_eq!(runs.get(), 1);
    let consumed = Err(format!(
        "no pending continuation for identity {}",
        own.get()
    ));
    assert_eq!(*repeated.borrow(), Some(consumed.clone()));
    let emitter = Emitter { key: 0 };
    assert_eq!(
        Exports::new(host).invoke_abort(own.get(), &emitter),
        consumed
    );
    assert_eq!(runs.get(), 1);
    assert!(signal.aborted());
    Ok(())
}

#[test]
fn signal_removal_prevents_later_notification() -> ExtensionResult<()> {
    let host = Controlled::default();
    let flag = Flag::default();
    let signal = host_signal(&host, &flag);
    let removed = Rc::new(Cell::new(0));
    let kept = Rc::new(Cell::new(0));
    let subscription = signal.subscribe(counting(&removed))?;
    let removed_id = latest(&host);
    subscription.unsubscribe()?;
    subscription.unsubscribe()?;
    assert_eq!(host.observed().bus.removals, [removed_id, removed_id]);
    drop(signal.subscribe(counting(&kept))?);

    assert_eq!(host.abort(&flag), [Ok(())]);
    assert_eq!((removed.get(), kept.get()), (0, 1));
    Ok(())
}

#[test]
fn dropped_execution_removes_its_signal_listener() -> ExtensionResult<()> {
    let host = Controlled::default();
    let api = captured_api(&host)?;
    let (hold, _started, _open) = gate();
    host.observe().hold_next_idle(hold);
    let options = ExecOptions {
        signal: Some(authored(host_signal(&host, &Flag::default()))),
        ..ExecOptions::default()
    };
    let mut execution = api.exec("held", &[], Some(options));
    let mut context = Context::from_waker(Waker::noop());
    assert!(execution.as_mut().poll(&mut context).is_pending());
    assert_eq!(host.observed().bus.listeners.len(), 1);
    drop(execution);
    assert!(
        host.observed().bus.listeners.is_empty(),
        "the dropped execution kept its signal listener"
    );
    Ok(())
}

#[test]
fn signal_late_subscription_does_not_replay() -> ExtensionResult<()> {
    let host = Controlled::default();
    let flag = Flag::default();
    let signal = host_signal(&host, &flag);
    assert!(host.abort(&flag).is_empty());
    let runs = Rc::new(Cell::new(0));
    let _late = signal.subscribe(counting(&runs))?;
    assert!(signal.aborted());
    assert_eq!(runs.get(), 0);
    Ok(())
}

#[test]
fn signal_rejected_admission_releases_captures() {
    let host = Controlled::default();
    let flag = Flag::default();
    let signal = host_signal(&host, &flag);
    host.observe().bus.registration_error = Some("subscription denied Ω".into());
    let guard = Reenter { host: host.clone() };
    let listener: AbortListener = Rc::new(move |_| {
        let _ = &guard;
        Ok(())
    });
    let identity = host.observed().dropped.len();
    assert_eq!(
        signal.subscribe(listener).err(),
        Some("subscription denied Ω".into())
    );
    let rejected = host.observed().dropped[identity];
    assert!(host.observed().bus.listeners.is_empty());
    assert_eq!(
        host.observed().bus.removals.len(),
        1,
        "the capture was released"
    );
    let emitter = Emitter { key: 0 };
    assert_eq!(
        Exports::new(host.clone()).invoke_abort(rejected, &emitter),
        Err(format!("no pending continuation for identity {rejected}"))
    );
    assert!(host.abort(&flag).is_empty());
}

#[test]
fn signal_reentry_never_borrows_callback_storage() -> ExtensionResult<()> {
    let host = Controlled::default();
    let flag = Flag::default();
    let signal = host_signal(&host, &flag);
    let inner = Rc::new(Cell::new(0));
    let effects = Rc::new(RefCell::new(Vec::new()));
    let listener: AbortListener = {
        let (host, signal, inner, effects) = (
            host.clone(),
            signal.clone(),
            Rc::clone(&inner),
            Rc::clone(&effects),
        );
        let guard = Reenter { host: host.clone() };
        Rc::new(move |emitter| {
            let _ = &guard;
            let registered = signal.subscribe(counting(&inner))?;
            let removed = events(host.clone()).on("inside", Rc::new(|_, _| Ok(None)))?;
            removed.unsubscribe()?;
            emitter.emit("from abort", Value::from("Ω"))?;
            effects
                .borrow_mut()
                .extend(host.observed().bus.emissions.iter().cloned());
            drop(registered);
            Ok(())
        })
    };
    let _subscription = signal.subscribe(listener)?;

    assert_eq!(host.abort(&flag), [Ok(()), Ok(())]);
    assert_eq!(
        *effects.borrow(),
        [("from abort".to_owned(), Value::from("Ω"))]
    );
    assert_eq!(inner.get(), 1);
    assert!(host.observed().bus.removals.len() >= 2);
    Ok(())
}
