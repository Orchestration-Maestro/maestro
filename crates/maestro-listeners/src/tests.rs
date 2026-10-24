//! Admission, removal and live traversal of one callback collection.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::{Admission, Listeners};

/// A callback and the count of its calls.
fn counted() -> (Rc<dyn Fn()>, Rc<Cell<usize>>) {
    let calls = Rc::new(Cell::new(0));
    let seen = Rc::clone(&calls);
    let callback: Rc<dyn Fn()> = Rc::new(move || seen.set(seen.get() + 1));
    (callback, calls)
}

/// Calls every live callback and reports how many calls the collection made.
fn dispatch(listeners: &RefCell<Listeners<Rc<dyn Fn()>>>) -> usize {
    let calls = Cell::new(0);
    Listeners::for_each_live(listeners, |_| calls.set(calls.get() + 1));
    calls.get()
}

#[test]
fn identity_admission_ignores_a_callback_it_already_holds() {
    let listeners = RefCell::new(Listeners::new(Admission::Once));
    let (callback, calls) = counted();
    assert!(listeners.borrow_mut().add(Rc::clone(&callback)).is_some());
    assert_eq!(listeners.borrow_mut().add(Rc::clone(&callback)), None);
    assert_eq!(dispatch(&listeners), 1);
    Listeners::for_each_live(&listeners, |live| live());
    assert_eq!(calls.get(), 1);
}

#[test]
fn every_registration_admission_keeps_both_registrations() {
    let listeners = RefCell::new(Listeners::new(Admission::Every));
    let (callback, calls) = counted();
    assert!(listeners.borrow_mut().add(Rc::clone(&callback)).is_some());
    assert!(listeners.borrow_mut().add(Rc::clone(&callback)).is_some());
    assert_eq!(dispatch(&listeners), 2);
    Listeners::for_each_live(&listeners, |live| live());
    assert_eq!(calls.get(), 2);
}

#[test]
fn subscribe_removes_only_the_registration_it_made() {
    let listeners = Rc::new(RefCell::new(Listeners::new(Admission::Every)));
    let (callback, calls) = counted();
    let first = Listeners::subscribe(&listeners, Rc::clone(&callback));
    let _second = Listeners::subscribe(&listeners, callback);
    first();
    assert_eq!(dispatch(&listeners), 1, "the later registration stays");
    first();
    assert_eq!(dispatch(&listeners), 1, "repeated removal does nothing");
    Listeners::for_each_live(&listeners, |live| live());
    assert_eq!(calls.get(), 1);
}

#[test]
fn removing_a_callback_admits_it_again() {
    let listeners = RefCell::new(Listeners::new(Admission::Once));
    let (callback, _calls) = counted();
    let ordinal = listeners
        .borrow_mut()
        .add(Rc::clone(&callback))
        .expect("the first registration is admitted");
    assert!(listeners.borrow_mut().remove_at(ordinal).is_some());
    assert!(listeners.borrow_mut().remove_at(ordinal).is_none());
    assert!(listeners.borrow_mut().add(callback).is_some());
    let (other, _) = counted();
    assert!(
        listeners.borrow_mut().remove(Rc::as_ptr(&other)).is_none(),
        "an unknown callback removes nothing"
    );
}
