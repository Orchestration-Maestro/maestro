//! A native listener cannot re-enter the component blocked on it, while other components run.
#![cfg(test)]
use super::retained_components::{loaded_on, view};
use maestro_extensions::{EventBus, Local, create_event_bus_for};
use std::{cell::RefCell, rc::Rc};

/// The one error a Store-blocking synchronous request gives every same-Store guest call.
const REENTRY: &str = "extension cannot be re-entered while it waits for a synchronous host call";

/// A bus whose tails and listener errors would fail the test.
fn bus() -> EventBus<Local> {
    create_event_bus_for::<Local>(
        Rc::new(|_| panic!("no tails")),
        Rc::new(|_| panic!("no errors")),
    )
    .for_extension()
}

#[test]
fn native_listener_reentry_reports_while_another_component_still_renders() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let (_owner, runner, events) = loaded_on(bus()).await;
        let (_other_owner, other_runner, other_events) = loaded_on(bus()).await;
        let retained = view(&runner, &events, "normal").unwrap().unwrap();
        let runner = Rc::new(runner);
        let observed = Rc::new(RefCell::new(Vec::new()));
        let (calls, record, listener_events) = (runner.clone(), observed.clone(), events.clone());
        let _subscription = events.on(
            "renderer-created",
            Rc::new(move |data, _| {
                if data[0] == "outer" {
                    let renderer = view(&calls, &listener_events, "inner").err();
                    let component = retained.try_render(10).err();
                    let other = view(&other_runner, &other_events, "normal")
                        .unwrap()
                        .unwrap()
                        .try_render(31)
                        .unwrap();
                    record.borrow_mut().push((
                        renderer.map(|error| error.to_string()),
                        component.map(|error| error.to_string()),
                        other,
                    ));
                }
                Ok(None)
            }),
        );
        let outer = view(&runner, &events, "outer").unwrap().unwrap();
        assert_eq!(
            *observed.borrow(),
            [(
                Some(REENTRY.to_owned()),
                Some(REENTRY.to_owned()),
                vec!["normal:31".to_owned()]
            )]
        );
        assert_eq!(outer.try_render(31).unwrap(), ["outer:31"]);
    });
}
