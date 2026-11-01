//! Synchronous abort observers on their owning thread.

use std::cell::RefCell;
use std::future::Future;
use std::rc::Rc;
use std::task::{Context, Poll, Waker};

use maestro_cancellation::Cancellation;

/// Ordered observations shared by the observers of one test.
type Trace = Rc<RefCell<Vec<&'static str>>>;
/// The removal operation a subscription returns.
type Removal = Box<dyn Fn()>;

/// Records its own release, proving captures drop outside the observer storage.
struct Capture(Trace, Cancellation);
impl Drop for Capture {
    fn drop(&mut self) {
        drop(self.1.subscribe(Rc::new(|| {})));
        self.0.borrow_mut().push("capture released");
    }
}

/// An observer that records `label` and checks the signal is already aborted.
fn record(trace: &Trace, signal: &Cancellation, label: &'static str) -> Rc<dyn Fn()> {
    let trace = Rc::clone(trace);
    let signal = signal.clone();
    Rc::new(move || {
        assert!(signal.is_aborted());
        trace.borrow_mut().push(label);
    })
}

/// Aborts `signal` between two trace marks.
fn abort_traced(trace: &Trace, signal: &Cancellation) {
    trace.borrow_mut().push("abort.before");
    signal.abort();
    trace.borrow_mut().push("abort.after");
}

/// Requires the signal to stay shareable across threads.
fn send_sync<T: Send + Sync>(_: &T) {}

#[test]
fn native_abort_observers_run_before_return_in_registration_order() {
    let trace = Trace::default();
    let signal = Cancellation::new();
    send_sync(&signal);
    let mut waiter = Box::pin(signal.cancelled());
    let mut context = Context::from_waker(Waker::noop());
    assert_eq!(waiter.as_mut().poll(&mut context), Poll::Pending);
    let _first = signal.subscribe(record(&trace, &signal, "a"));
    let _second = signal.clone().subscribe(record(&trace, &signal, "b"));

    abort_traced(&trace, &signal);
    assert_eq!(*trace.borrow(), ["abort.before", "a", "b", "abort.after"]);
    assert_eq!(waiter.as_mut().poll(&mut context), Poll::Ready(()));

    abort_traced(&trace, &signal);
    assert_eq!(
        *trace.borrow(),
        [
            "abort.before",
            "a",
            "b",
            "abort.after",
            "abort.before",
            "abort.after"
        ]
    );
}

#[test]
fn native_abort_observers_keep_live_edits_and_explicit_removal() {
    let trace = Trace::default();
    let signal = Cancellation::new();
    let shared = record(&trace, &signal, "a");
    let _first = signal.subscribe(Rc::clone(&shared));
    let duplicate = signal.subscribe(shared);
    let removed = signal.subscribe(record(&trace, &signal, "removed"));
    removed();
    removed();
    drop(signal.subscribe(record(&trace, &signal, "dropped remover")));

    let later: Rc<RefCell<Option<Removal>>> = Rc::default();
    let editor = {
        let (trace, signal, later) = (Rc::clone(&trace), signal.clone(), Rc::clone(&later));
        Rc::new(move || {
            trace.borrow_mut().push("editor");
            if let Some(remove) = later.borrow_mut().take() {
                remove();
            }
            drop(signal.subscribe(record(&trace, &signal, "fresh")));
        })
    };
    let _editor = signal.subscribe(editor);
    *later.borrow_mut() = Some(signal.subscribe(record(&trace, &signal, "later")));
    let _after = signal.subscribe(record(&trace, &signal, "after editor"));
    drop(duplicate);

    abort_traced(&trace, &signal);
    assert_eq!(
        *trace.borrow(),
        [
            "abort.before",
            "a",
            "dropped remover",
            "editor",
            "after editor",
            "fresh",
            "abort.after"
        ]
    );
}

#[test]
fn native_abort_reentry_is_once_and_does_not_hold_observer_storage() {
    let trace = Trace::default();
    let signal = Cancellation::new();
    let reentrant = {
        let (trace, signal) = (Rc::clone(&trace), signal.clone());
        let capture = Capture(Rc::clone(&trace), signal.clone());
        Rc::new(move || {
            let _held = &capture;
            trace.borrow_mut().push("reentrant");
            signal.abort();
            let removal = signal.subscribe(record(&trace, &signal, "registered inside"));
            let removed = signal.subscribe(record(&trace, &signal, "removed inside"));
            removed();
            drop(removal);
            trace.borrow_mut().push("reentrant returned");
        })
    };
    let remove = signal.subscribe(reentrant);
    let _next = signal.subscribe(record(&trace, &signal, "next"));

    abort_traced(&trace, &signal);
    assert_eq!(
        *trace.borrow(),
        [
            "abort.before",
            "reentrant",
            "reentrant returned",
            "next",
            "registered inside",
            "capture released",
            "abort.after"
        ]
    );
    remove();
}

#[test]
fn native_abort_subscription_does_not_replay_a_prior_abort() {
    let trace = Trace::default();
    let signal = Cancellation::new();
    let adder = {
        let (trace, signal) = (Rc::clone(&trace), signal.clone());
        Rc::new(move || drop(signal.subscribe(record(&trace, &signal, "added during"))))
    };
    let _adder = signal.subscribe(adder);
    abort_traced(&trace, &signal);

    let late = signal.subscribe(record(&trace, &signal, "late"));
    abort_traced(&trace, &signal);
    late();
    assert!(signal.is_aborted());
    assert_eq!(
        *trace.borrow(),
        [
            "abort.before",
            "added during",
            "abort.after",
            "abort.before",
            "abort.after"
        ]
    );
}
