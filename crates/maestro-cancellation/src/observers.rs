//! Synchronous abort observers kept on the thread that registered them.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    sync::{Arc, Weak},
};

use maestro_listeners::Listeners;

use crate::Shared;

/// One registered observer; its identity is its allocation.
type Observer = Rc<dyn Fn()>;

/// This thread's observers of one signal that have not run yet.
struct Observers {
    /// Reserves the signal's allocation, so its address keys no other signal.
    signal: Weak<Shared>,
    /// Observers in registration order, walked live.
    list: Rc<RefCell<Listeners<Observer>>>,
    /// Set while the walk runs, so a nested abort does not walk again.
    walking: Cell<bool>,
}

/// Signals with observers on this thread, keyed by their shared allocation.
#[derive(Default)]
struct Registry {
    /// Pending observers of each signal.
    signals: HashMap<usize, Rc<Observers>>,
    /// Signal count at which entries of dropped signals are discarded, doubling so the
    /// work stays linear.
    prune_at: usize,
}

thread_local! {
    /// Observers registered on this thread.
    static REGISTRY: RefCell<Registry> = RefCell::default();
}

/// The key of one shared signal.
fn key(shared: &Arc<Shared>) -> usize {
    Arc::as_ptr(shared).addr()
}

/// This thread's pending observers of `shared`, if any.
fn find(shared: &Arc<Shared>) -> Option<Rc<Observers>> {
    REGISTRY.with_borrow(|registry| registry.signals.get(&key(shared)).cloned())
}

/// Adds `observer` unless the signal was already aborted outside a walk of this thread.
pub(crate) fn subscribe(shared: &Arc<Shared>, observer: Observer) -> Box<dyn Fn()> {
    let observers = match find(shared) {
        Some(observers) if observers.walking.get() || !shared.is_aborted() => observers,
        Some(_) => return Box::new(|| {}),
        None if shared.is_aborted() => return Box::new(|| {}),
        None => insert(shared),
    };
    Box::new(Listeners::subscribe(&observers.list, observer))
}

/// Creates this thread's entry for `shared`, discarding entries of dropped signals.
fn insert(shared: &Arc<Shared>) -> Rc<Observers> {
    let observers = Rc::new(Observers {
        signal: Arc::downgrade(shared),
        list: Rc::default(),
        walking: Cell::new(false),
    });
    let discarded: Vec<_> = REGISTRY.with_borrow_mut(|registry| {
        registry.signals.insert(key(shared), Rc::clone(&observers));
        if registry.signals.len() < registry.prune_at {
            return Vec::new();
        }
        let discarded = registry
            .signals
            .extract_if(|_, observers| observers.signal.strong_count() == 0)
            .collect();
        registry.prune_at = registry.signals.len() * 2;
        discarded
    });
    drop(discarded);
    observers
}

/// Runs this thread's observers of `shared` once, in registration order.
///
/// The walk sees registrations and removals made by the observers themselves. The
/// observers are released once the walk ends, outside every borrow.
pub(crate) fn deliver(shared: &Arc<Shared>) {
    let Some(observers) = find(shared) else {
        return;
    };
    if observers.walking.replace(true) {
        return;
    }
    let Ok(()) = Listeners::try_for_each_live(&observers.list, |observer| {
        observer();
        Ok::<(), std::convert::Infallible>(())
    });
    let removed = REGISTRY.with_borrow_mut(|registry| registry.signals.remove(&key(shared)));
    drop(removed);
}

/// Releases this thread's observers of a signal being dropped here.
pub(crate) fn release(shared: &Shared) {
    let key = std::ptr::from_ref(shared).addr();
    let removed = REGISTRY
        .try_with(|registry| {
            registry
                .try_borrow_mut()
                .ok()
                .and_then(|mut registry| registry.signals.remove(&key))
        })
        .ok()
        .flatten();
    drop(removed);
}
