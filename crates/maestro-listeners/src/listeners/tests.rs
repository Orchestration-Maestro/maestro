//! Registration order, identity admission and traversal work of the collection.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::{Admission, Listeners, reset_visits, visits};

/// A shared collection of identity payloads, as its runtime callers hold it.
type Shared = Rc<RefCell<Listeners<Rc<usize>>>>;

/// A callback that records its label and the text it received.
type Callback = Rc<dyn Fn(&str) -> Result<(), ()>>;

/// A shared collection of callbacks.
type Registry = Rc<RefCell<Listeners<Callback>>>;

/// The visits recorded by one traversal.
type Log = Rc<RefCell<Vec<(usize, String)>>>;

/// An empty shared collection of identity payloads.
fn shared() -> Shared {
    Rc::new(RefCell::new(Listeners::default()))
}

/// The values of one traversal over a shared collection, in visit order.
fn walk(shared: &Shared) -> Vec<usize> {
    let mut seen = Vec::new();
    let outcome = Listeners::try_for_each_live(shared, |value| {
        seen.push(*value);
        Ok::<(), ()>(())
    });
    assert_eq!(outcome, Ok(()));
    seen
}

/// Register `value` and return its remover.
fn subscribe(shared: &Shared, value: &Rc<usize>) -> Box<dyn Fn()> {
    Box::new(Listeners::subscribe(shared, Rc::clone(value)))
}

/// The values of one traversal over an unsynchronized registry, in visit order.
fn walk_owned(listeners: &mut Listeners<Rc<usize>>) -> Vec<usize> {
    let mut cursor = listeners.cursor();
    let mut seen = Vec::new();
    while let Some(value) = listeners.next_from(&mut cursor) {
        seen.push(**value);
    }
    seen
}

/// Remove every registration of `values` in the order they arrive.
fn clear(listeners: &mut Listeners<Rc<usize>>, values: impl Iterator<Item = Rc<usize>>) {
    for value in values {
        drop(listeners.remove(Rc::as_ptr(&value)));
    }
}

/// A callback for `label` that records its visit and then runs `action`.
fn recording(label: usize, log: &Log, action: impl Fn(&str) + 'static) -> Callback {
    let log = Rc::clone(log);
    Rc::new(move |text| {
        log.borrow_mut().push((label, text.to_owned()));
        action(text);
        Ok(())
    })
}

/// A callback for `label` that only records its visit.
fn plain(label: usize, log: &Log) -> Callback {
    recording(label, log, |_| {})
}

/// A recording callback for `label` that can remove or re-register itself.
fn self_referencing(
    label: usize,
    log: &Log,
    action: impl Fn(&Callback) + 'static,
) -> (Rc<RefCell<Option<Callback>>>, Callback) {
    let log = Rc::clone(log);
    let holder: Rc<RefCell<Option<Callback>>> = Rc::default();
    let weak = Rc::downgrade(&holder);
    let callback: Callback = Rc::new(move |text| {
        log.borrow_mut().push((label, text.to_owned()));
        if let Some(this) = weak.upgrade().and_then(|holder| holder.borrow().clone()) {
            action(&this);
        }
        Ok(())
    });
    *holder.borrow_mut() = Some(Rc::clone(&callback));
    (holder, callback)
}

/// Walk `registry` with `text`.
fn dispatch(registry: &Registry, text: &str) {
    let outcome = Listeners::try_for_each_live(registry, |callback| callback(text));
    assert_eq!(outcome, Ok(()));
}

/// The labels one recorded traversal visited, in order.
fn labels(log: &Log) -> Vec<usize> {
    log.borrow().iter().map(|(label, _)| *label).collect()
}

/// The collection a callback registers into, held weakly so the callback registered in it
/// does not keep it alive.
fn registrar(registry: &Registry) -> impl Fn() -> Registry + 'static {
    let weak = Rc::downgrade(registry);
    move || weak.upgrade().expect("the collection outlives its walk")
}

/// A collection with callback 1 adding a tail callback while it runs.
fn live_add(log: &Log) -> Registry {
    let registry: Registry = Rc::default();
    let appended = plain(2, log);
    let registrar = registrar(&registry);
    registry.borrow_mut().add(recording(1, log, move |_| {
        registrar().borrow_mut().add(Rc::clone(&appended));
    }));
    registry
}

/// A collection with callback 1 removing an unvisited callback.
fn live_remove(log: &Log) -> Registry {
    let registry: Registry = Rc::default();
    let skipped = plain(2, log);
    let registrar = registrar(&registry);
    let removed = Rc::clone(&skipped);
    registry.borrow_mut().add(recording(1, log, move |_| {
        let removed = registrar().borrow_mut().remove(Rc::as_ptr(&removed));
        drop(removed);
    }));
    registry.borrow_mut().add(skipped);
    registry
}

/// A collection whose first callback removes itself during its own visit.
fn self_removing(log: &Log) -> (Registry, Rc<RefCell<Option<Callback>>>) {
    let registry: Registry = Rc::default();
    let registrar = registrar(&registry);
    let (holder, callback) = self_referencing(1, log, move |this| {
        let removed = registrar().borrow_mut().remove(Rc::as_ptr(this));
        drop(removed);
    });
    registry.borrow_mut().add(callback);
    registry.borrow_mut().add(plain(3, log));
    (registry, holder)
}

/// A collection whose first callback moves an unvisited callback to the tail.
fn readd_moves_to_tail(log: &Log) -> Registry {
    let registry: Registry = Rc::default();
    let moved = plain(2, log);
    let tail = plain(3, log);
    let registrar = registrar(&registry);
    let moved_again = Rc::clone(&moved);
    registry.borrow_mut().add(recording(1, log, move |_| {
        let registry = registrar();
        let mut listeners = registry.borrow_mut();
        drop(listeners.remove(Rc::as_ptr(&moved_again)));
        listeners.add(Rc::clone(&moved_again));
    }));
    registry.borrow_mut().add(Rc::clone(&moved));
    registry.borrow_mut().add(Rc::clone(&tail));
    registry
}

/// A collection whose first callback re-registers itself once.
fn self_readd_once(log: &Log) -> (Registry, Rc<RefCell<Option<Callback>>>) {
    let registry: Registry = Rc::default();
    let registrar = registrar(&registry);
    let once = Cell::new(false);
    let (holder, callback) = self_referencing(1, log, move |this| {
        if once.replace(true) {
            return;
        }
        let registry = registrar();
        let mut listeners = registry.borrow_mut();
        drop(listeners.remove(Rc::as_ptr(this)));
        listeners.add(Rc::clone(this));
    });
    registry.borrow_mut().add(callback);
    registry.borrow_mut().add(plain(2, log));
    (registry, holder)
}

/// A collection whose first callback walks the same collection once.
fn nested_dispatch(log: &Log) -> Registry {
    let registry: Registry = Rc::default();
    let nested = registrar(&registry);
    registry.borrow_mut().add(recording(1, log, move |text| {
        if text == "original" {
            dispatch(&nested(), "nested");
        }
    }));
    registry.borrow_mut().add(plain(2, log));
    registry
}

/// A callback that observes whether its collection is borrowed when it is released.
struct Reentrant {
    /// What the release observed; `None` until the callback is dropped.
    released: Rc<Cell<Option<bool>>>,
    /// The collection holding the registration.
    registry: std::rc::Weak<RefCell<Listeners<Rc<Reentrant>>>>,
}

impl Drop for Reentrant {
    fn drop(&mut self) {
        let borrowed = self
            .registry
            .upgrade()
            .map(|registry| registry.try_borrow().is_ok());
        self.released.set(borrowed);
    }
}

#[test]
fn identity_admission_keeps_live_order() {
    let mut listeners: Listeners<Rc<usize>> = Listeners::default();
    assert_eq!(walk_owned(&mut listeners), Vec::<usize>::new());

    let (first, second) = (Rc::new(1), Rc::new(2));
    listeners.add(Rc::clone(&second));
    listeners.add(Rc::clone(&first));
    assert_eq!(walk_owned(&mut listeners), [2, 1]);

    let (left, right) = (Rc::new(7), Rc::new(7));
    listeners.add(Rc::clone(&left));
    listeners.add(Rc::clone(&right));
    assert_eq!(walk_owned(&mut listeners), [2, 1, 7, 7]);

    listeners.add(Rc::clone(&left));
    assert_eq!(walk_owned(&mut listeners), [2, 1, 7, 7]);

    let mut foreign: Listeners<Rc<usize>> = Listeners::default();
    foreign.add(Rc::new(9));
    let mut cursor = foreign.cursor();
    assert!(listeners.next_from(&mut cursor).is_none());
}

#[test]
fn removers_follow_identity_and_release_outside_borrows() {
    for which in [0, 1] {
        let value = Rc::new(4);
        let weak = Rc::downgrade(&value);
        let registry = shared();
        let removers = [subscribe(&registry, &value), subscribe(&registry, &value)];
        assert_eq!(walk(&registry), [4]);
        removers[which]();
        removers[which]();
        assert!(registry.borrow().identities.is_empty());
        drop(value);
        assert!(weak.upgrade().is_none());
        removers[1 - which]();
    }

    let registry = shared();
    let value = Rc::new(5);
    let old = subscribe(&registry, &value);
    assert_eq!(walk(&registry), [5]);
    let removed = registry.borrow_mut().remove(Rc::as_ptr(&value));
    drop(removed);
    drop(subscribe(&registry, &value));
    old();
    assert_eq!(walk(&registry), Vec::<usize>::new());

    let registry = shared();
    let kept = Rc::new(6);
    let dropped = subscribe(&registry, &kept);
    drop(dropped);
    assert_eq!(walk(&registry), [6]);

    let registry = shared();
    let unknown = Rc::new(7);
    assert_eq!(registry.borrow_mut().remove(Rc::as_ptr(&unknown)), None);

    let registry: Rc<RefCell<Listeners<Rc<Reentrant>>>> = Rc::default();
    let released = Rc::new(Cell::new(None));
    let probe = Rc::new(Reentrant {
        released: Rc::clone(&released),
        registry: Rc::downgrade(&registry),
    });
    registry.borrow_mut().add(Rc::clone(&probe));
    let removed = registry.borrow_mut().remove(Rc::as_ptr(&probe));
    drop(removed);
    drop(probe);
    assert_eq!(released.get(), Some(true));
}

/// A capture whose release unsubscribes another registration.
struct Unsubscribing(Option<Box<dyn Fn()>>);

impl Drop for Unsubscribing {
    fn drop(&mut self) {
        if let Some(unsubscribe) = &self.0 {
            unsubscribe();
        }
    }
}

#[test]
fn removal_releases_a_capture_that_unsubscribes_another() {
    for admission in [Admission::Identity, Admission::Every] {
        let registry = Rc::new(RefCell::new(Listeners::new(admission)));
        let other = Rc::new(Unsubscribing(None));
        let released = Rc::downgrade(&other);
        let unsubscribe_other = Listeners::subscribe(&registry, other);
        let capture = Rc::new(Unsubscribing(Some(Box::new(unsubscribe_other))));
        Listeners::subscribe(&registry, capture)();
        assert!(
            released.upgrade().is_none(),
            "the other registration is released"
        );
        assert!(registry.borrow().identities.is_empty());
    }
}

#[test]
fn every_registration_admission_removes_each_registration_once() {
    let registry: Shared = Rc::new(RefCell::new(Listeners::new(Admission::Every)));
    let value = Rc::new(3);
    let removers: Vec<_> = (0..4).map(|_| subscribe(&registry, &value)).collect();
    assert_eq!(walk(&registry), [3, 3, 3, 3]);
    assert_eq!(registry.borrow_mut().remove(Rc::as_ptr(&value)), None);
    for remover in &removers[..3] {
        remover();
        remover();
    }
    assert_eq!(walk(&registry), [3]);
    removers[3]();
    assert_eq!(walk(&registry), Vec::<usize>::new());
    assert!(registry.borrow().entries.is_empty());
}

/// Drops `registry` and checks that no callback outlived it: the collection is freed and
/// the callbacks released the `log` they recorded into.
fn assert_released(log: &Log, registry: Registry) {
    let weak = Rc::downgrade(&registry);
    drop(registry);
    assert!(weak.upgrade().is_none(), "a callback keeps its collection");
    assert_eq!(
        Rc::strong_count(log),
        1,
        "a callback outlives its collection"
    );
}

#[test]
fn live_walks_observe_remove_append_and_readd() {
    let log: Log = Rc::default();
    let registry = live_add(&log);
    dispatch(&registry, "original");
    assert_eq!(labels(&log), [1, 2]);
    assert_released(&log, registry);

    let log: Log = Rc::default();
    let registry = live_remove(&log);
    dispatch(&registry, "original");
    assert_eq!(labels(&log), [1]);
    assert_released(&log, registry);

    let log: Log = Rc::default();
    let (registry, holder) = self_removing(&log);
    dispatch(&registry, "original");
    assert_eq!(labels(&log), [1, 3]);
    drop(holder);
    assert_released(&log, registry);

    let log: Log = Rc::default();
    let registry = readd_moves_to_tail(&log);
    dispatch(&registry, "original");
    assert_eq!(labels(&log), [1, 3, 2]);
    assert_released(&log, registry);

    let log: Log = Rc::default();
    let (registry, holder) = self_readd_once(&log);
    dispatch(&registry, "original");
    assert_eq!(labels(&log), [1, 2, 1]);
    drop(holder);
    assert_released(&log, registry);

    let log: Log = Rc::default();
    let registry = nested_dispatch(&log);
    dispatch(&registry, "original");
    assert_eq!(
        *log.borrow(),
        [
            (1, "original".to_owned()),
            (1, "nested".to_owned()),
            (2, "nested".to_owned()),
            (2, "original".to_owned()),
        ]
    );
    assert_released(&log, registry);
}

#[test]
fn early_exit_releases_cursor_and_preserves_first_error() {
    let registry = shared();
    let values: Vec<_> = (1..=3).map(Rc::new).collect();
    for value in &values {
        drop(subscribe(&registry, value));
    }
    let mut seen = Vec::new();
    let outcome = Listeners::try_for_each_live(&registry, |value| {
        seen.push(*value);
        if *value == 2 { Err("stop") } else { Ok(()) }
    });
    assert_eq!(outcome, Err("stop"));
    assert_eq!(seen, [1, 2]);
    assert_eq!(registry.borrow().active.load(Ordering::Relaxed), 0);
    let removed = registry.borrow_mut().remove(Rc::as_ptr(&values[2]));
    drop(removed);
    reset_visits();
    assert_eq!(walk(&registry), [1, 2]);
    assert!(visits() <= 3);
}

/// One end of the registrations a clear starts from.
#[derive(Clone, Copy)]
enum End {
    /// The first registration.
    Front,
    /// The last registration.
    Back,
}

/// Clear `count` registrations from `end` and check the work it took.
fn clear_all(retained: bool, end: End, count: usize) {
    let mut listeners: Listeners<Rc<usize>> = Listeners::default();
    let values: Vec<_> = (0..count).map(Rc::new).collect();
    for value in &values {
        listeners.add(Rc::clone(value));
    }
    let cursor = retained.then(|| listeners.cursor());
    reset_visits();
    match end {
        End::Front => clear(&mut listeners, values.iter().cloned()),
        End::Back => clear(&mut listeners, values.iter().rev().cloned()),
    }
    if retained {
        assert_eq!(listeners.entries.len(), count);
    }
    drop(cursor);
    assert_eq!(walk_owned(&mut listeners), Vec::<usize>::new());
    let counted = visits();
    assert!(counted <= 3 * count, "{count}: {counted}");
    assert!(listeners.entries.is_empty());
}

#[test]
fn bulk_removal_has_linear_work() {
    for retained in [false, true] {
        for end in [End::Front, End::Back] {
            for count in [128, 256, 512] {
                clear_all(retained, end, count);
            }
        }
    }
}

/// The values of a walk that restarts a traversal for every registration, the way a
/// caller without a cursor rescanning the collection for each listener would.
fn rescan(listeners: &mut Listeners<Rc<usize>>) -> Vec<usize> {
    let mut seen = Vec::new();
    for index in 0..listeners.identities.len() {
        let mut cursor = listeners.cursor();
        let mut value = None;
        for _ in 0..=index {
            value = listeners.next_from(&mut cursor).map(|listener| **listener);
        }
        seen.extend(value);
    }
    seen
}

#[test]
fn widget_map_walk_has_linear_slot_work() {
    for count in [128, 256, 512] {
        let mut listeners: Listeners<Rc<usize>> = Listeners::default();
        for slot in 0..count {
            listeners.add(Rc::new(slot));
        }
        reset_visits();
        let seen = walk_owned(&mut listeners);
        let counted = visits();
        assert_eq!(seen.len(), count);
        assert!(counted <= count + 1, "{count}: {counted}");
        reset_visits();
        assert_eq!(rescan(&mut listeners), seen);
        let rescanned = visits();
        assert!(rescanned > count + 1, "{count}: {rescanned}");
        let mut cursor = listeners.cursor();
        reset_visits();
        let mut observed = 0;
        while let Some(value) = listeners.next_from(&mut cursor).cloned() {
            observed += 1;
            drop(listeners.remove(Rc::as_ptr(&value)));
            if *value == 0 {
                listeners.add(Rc::new(count));
            }
        }
        assert_eq!(observed, count + 1);
        assert!(visits() <= 3 * count + 4);
        drop(cursor);
        reset_visits();
        assert!(walk_owned(&mut listeners).is_empty());
        assert!(visits() <= count + 1);
    }
}

#[test]
fn remover_after_registry_drop_does_nothing() {
    let registry = shared();
    let value = Rc::new(8);
    let remover = subscribe(&registry, &value);
    drop(registry);
    remover();
    assert_eq!(Rc::strong_count(&value), 1);
}

#[test]
fn cursor_keeps_no_capture_of_a_removed_callback() {
    let mut listeners: Listeners<Rc<usize>> = Listeners::default();
    let value = Rc::new(2);
    let weak = Rc::downgrade(&value);
    listeners.add(Rc::clone(&value));
    let cursor = listeners.cursor();
    drop(listeners.remove(Rc::as_ptr(&value)));
    drop(value);
    assert!(weak.upgrade().is_none());
    drop(cursor);
}

#[test]
fn abandoned_traversal_compacts_retired_slots() {
    let mut listeners: Listeners<Rc<usize>> = Listeners::default();
    let values: Vec<_> = (0..2).map(Rc::new).collect();
    for value in &values {
        listeners.add(Rc::clone(value));
    }
    let cursor = listeners.cursor();
    drop(listeners.remove(Rc::as_ptr(&values[0])));
    assert_eq!(listeners.entries.len(), 2);
    drop(cursor);
    assert_eq!(
        listeners.entries.len(),
        2,
        "reclaiming waits for the next operation"
    );
    listeners.add(Rc::new(9));
    assert_eq!(listeners.entries.len(), 2);
    assert_eq!(walk_owned(&mut listeners), [1, 9]);
}

#[test]
fn membership_sequences_match_the_order_model() {
    let values: Vec<_> = (0..32).map(Rc::new).collect();
    let mut listeners: Listeners<Rc<usize>> = Listeners::default();
    let mut expected: Vec<usize> = Vec::new();
    let mut seed = 1_u64;
    for _ in 0..100_000 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let index = ((seed >> 32) % 32) as usize;
        if seed & 1 == 0 {
            listeners.add(Rc::clone(&values[index]));
            if !expected.contains(&index) {
                expected.push(index);
            }
        } else {
            drop(listeners.remove(Rc::as_ptr(&values[index])));
            expected.retain(|value| *value != index);
        }
        assert_eq!(walk_owned(&mut listeners), expected);
        assert!(listeners.entries.len() <= 2 * values.len() + 1);
    }
}

#[test]
fn cursor_can_cross_a_threaded_await() {
    fn threaded<T: Send>(_: T) {}
    fn shared_collection<T: Send + Sync>() {}

    shared_collection::<Listeners<Arc<usize>>>();
    let mut listeners: Listeners<Arc<usize>> = Listeners::default();
    listeners.add(Arc::new(3));
    let mut cursor = listeners.cursor();
    let value = listeners.next_from(&mut cursor).map(Arc::clone);
    threaded(async move {
        std::future::ready(()).await;
        drop(cursor);
        value
    });
}

#[test]
fn retained_snapshots_keep_membership_order_and_owned_values() {
    let mut listeners = Listeners::default();
    assert!(listeners.snapshot().is_empty());
    let first = Rc::new(7);
    let second = Rc::new(3);
    let weak = Rc::downgrade(&first);
    listeners.add(Rc::clone(&first));
    listeners.add(Rc::clone(&second));
    let snapshot = listeners.snapshot();
    assert!(Rc::ptr_eq(&snapshot[0], &first));
    assert!(Rc::ptr_eq(&snapshot[1], &second));
    drop(listeners.remove(Rc::as_ptr(&first)));
    listeners.add(Rc::new(9));
    assert_eq!(
        listeners
            .snapshot()
            .iter()
            .map(|value| **value)
            .collect::<Vec<_>>(),
        [3, 9]
    );
    drop(listeners);
    drop(first);
    drop(second);
    assert_eq!(
        snapshot.iter().map(|value| **value).collect::<Vec<_>>(),
        [7, 3]
    );
    assert!(weak.upgrade().is_some());
    drop(snapshot);
    assert!(weak.upgrade().is_none());
}

#[test]
fn retained_snapshot_walk_keeps_linear_work() {
    for count in [128, 256, 512] {
        let mut listeners = Listeners::default();
        for value in 0..count {
            listeners.add(Rc::new(value));
        }
        reset_visits();
        let snapshot = listeners.snapshot();
        assert_eq!(
            snapshot.iter().map(|value| **value).collect::<Vec<_>>(),
            (0..count).collect::<Vec<_>>()
        );
        assert!(visits() <= count + 1, "{count}: {}", visits());
    }
}
