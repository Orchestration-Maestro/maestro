//! Listener identity registration and snapshot tagging stay linear in the listener count.
use super::{super::Frame, super::Tail, CAPACITY, INSPECTIONS};
use crate::local_resources::Pump;
use maestro_extensions::{EventListener, Local, create_event_bus_for};
use std::rc::Rc;

/// Identities inspected while registering `count` live and `count` retired listeners, then
/// tagging one emission's snapshot of the live ones and one foreign listener.
fn inspections(count: u32) -> (usize, Vec<Option<u32>>) {
    let (pump, _admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    let events = create_event_bus_for::<Local>(Rc::new(|_| {}), Rc::new(|_| {})).for_extension();
    INSPECTIONS.set(0);
    let mut subscriptions = Vec::new();
    for id in 0..count {
        let listener: EventListener<Local> = Rc::new(|_, _| Ok(None));
        registry.listener_identity(&listener, id);
        subscriptions.push(events.on("channel", listener));
        let retired: EventListener<Local> = Rc::new(|_, _| Ok(None));
        registry.listener_identity(&retired, count + id);
    }
    let foreign: EventListener<Local> = Rc::new(|_, _| Ok(None));
    let _foreign = events.on("channel", foreign);
    let (_, tags) = events.invoke_callback(|emitter| {
        let frame = Frame {
            native: emitter.frame(),
            snapshots: std::cell::RefCell::default(),
            local: std::cell::RefCell::default(),
            tail: std::cell::RefCell::new(Tail::Open(None)),
        };
        registry.snapshot(&frame, "channel", "null").unwrap()
    });
    (INSPECTIONS.get(), tags)
}

#[test]
fn listener_identities_register_and_tag_with_linear_work() {
    let mut work = Vec::new();
    for count in [256, 512] {
        let (inspected, tags) = inspections(count);
        let expected: Vec<_> = (0..count).map(Some).chain([None]).collect();
        assert_eq!(tags, expected);
        work.push(inspected);
    }
    assert!(work[1] < 3 * work[0], "work at N and 2N: {work:?}");
}

/// Map capacity scanned by pruning while `count` listeners register and retire together,
/// then `2 * count` more register and retire one at a time.
fn pruned_capacity(count: usize) -> usize {
    let (pump, _admission) = Pump::new(crate::tests::support::widgets());
    let registry = pump.registry();
    CAPACITY.set(0);
    let burst: Vec<EventListener<Local>> = (0..count)
        .map(|_| {
            let listener: EventListener<Local> = Rc::new(|_, _| Ok(None));
            listener
        })
        .collect();
    for listener in &burst {
        registry.listener_identity(listener, 0);
    }
    drop(burst);
    for _ in 0..2 * count {
        let listener: EventListener<Local> = Rc::new(|_, _| Ok(None));
        registry.listener_identity(&listener, 0);
    }
    CAPACITY.get()
}

#[test]
fn listener_identity_pruning_scans_linear_capacity_after_a_burst() {
    let (small, large) = (pruned_capacity(128), pruned_capacity(256));
    assert!(
        large < 3 * small,
        "pruning scanned {small} then {large} buckets for 128 then 256 listeners"
    );
}
