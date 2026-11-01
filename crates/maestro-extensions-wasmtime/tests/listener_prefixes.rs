//! Guest-local prefixes keep the native diagnostic and independent tail boundary.
#![cfg(test)]
use maestro_extensions::{Local, create_event_bus_for};
use std::{cell::RefCell, rc::Rc};

use super::retained_components::{loaded_on, view};

#[test]
fn guest_local_listener_errors_report_once_and_do_not_skip_later_listeners() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let errors = Rc::new(RefCell::new(Vec::new()));
        let reported = errors.clone();
        let bus = create_event_bus_for::<Local>(
            Rc::new(|_| panic!("no tails")),
            Rc::new(move |error| reported.borrow_mut().push(error.to_owned())),
        );
        let trace = Rc::new(RefCell::new(Vec::new()));
        let observed = trace.clone();
        let events = bus.for_extension();
        events.on(
            "trace",
            Rc::new(move |data, _| {
                observed
                    .borrow_mut()
                    .push(data.as_str().unwrap().to_owned());
                Ok(None)
            }),
        );
        let (_owner, runner, events) = loaded_on(events).await;
        let _component = view(&runner, &events, "listener-error").unwrap().unwrap();
        assert_eq!(*trace.borrow(), ["g1:listener-error", "g2:listener-error"]);
        assert_eq!(
            *errors.borrow(),
            ["Event handler error (renderer-created): guest listener failure\n"]
        );
    });
}

#[test]
fn guest_local_listener_tails_are_owned_separately_and_do_not_run_before_prefix_return() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let tails = Rc::new(RefCell::new(Vec::new()));
        let queued = tails.clone();
        let bus = create_event_bus_for::<Local>(
            Rc::new(move |tail| queued.borrow_mut().push(tail)),
            Rc::new(|error| panic!("{error}")),
        );
        let trace = Rc::new(RefCell::new(Vec::new()));
        let observed = trace.clone();
        let events = bus.for_extension();
        events.on(
            "trace",
            Rc::new(move |data, _| {
                observed
                    .borrow_mut()
                    .push(data.as_str().unwrap().to_owned());
                Ok(None)
            }),
        );
        let (_owner, runner, events) = loaded_on(events).await;
        let _component = view(&runner, &events, "listener-tail").unwrap().unwrap();
        assert_eq!(*trace.borrow(), ["g1:listener-tail", "g2:listener-tail"]);
        let pending = std::mem::take(&mut *tails.borrow_mut());
        assert_eq!(pending.len(), 2);
        for tail in pending {
            tail.await;
        }
        assert_eq!(
            *trace.borrow(),
            ["g1:listener-tail", "g2:listener-tail", "g1:tail", "g2:tail"]
        );
    });
}
