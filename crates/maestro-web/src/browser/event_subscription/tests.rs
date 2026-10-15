//! Native subscription independence witness, qualified on browser hosts.
use super::EventSubscription;
use std::{cell::Cell, rc::Rc};
use wasm_bindgen::closure::Closure;
use web_sys::{Event, EventTarget};

/// Removing one registration leaves its sibling delivering on the same target.
#[test]
fn subscriptions_detach_independently() {
    let target = EventTarget::new().unwrap();
    let first_calls = Rc::new(Cell::new(0));
    let second_calls = Rc::new(Cell::new(0));
    let subscribe = |calls: &Rc<Cell<usize>>| {
        let calls = Rc::clone(calls);
        EventSubscription::new(
            target.clone(),
            "message",
            Closure::wrap(
                Box::new(move |_: Event| calls.set(calls.get() + 1)) as Box<dyn FnMut(Event)>
            ),
        )
        .unwrap()
    };
    let first = subscribe(&first_calls);
    let second = subscribe(&second_calls);
    target
        .dispatch_event(&Event::new("message").unwrap())
        .unwrap();
    assert_eq!((first_calls.get(), second_calls.get()), (1, 1));
    first.detach().unwrap();
    target
        .dispatch_event(&Event::new("message").unwrap())
        .unwrap();
    assert_eq!((first_calls.get(), second_calls.get()), (1, 2));
    drop(first);
    target
        .dispatch_event(&Event::new("message").unwrap())
        .unwrap();
    assert_eq!((first_calls.get(), second_calls.get()), (1, 3));
    drop(second);
    target
        .dispatch_event(&Event::new("message").unwrap())
        .unwrap();
    assert_eq!((first_calls.get(), second_calls.get()), (1, 3));
}
