//! Owned repeating callbacks through the controlled terminal runtime.
use crate::support::manual_runtime::{ManualRuntime, ms};
use maestro_tui::tui::RepeatingTimer;
use std::{cell::Cell, rc::Rc};

#[test]
fn repeating_timer_repeats_until_owner_drop() {
    let runtime = ManualRuntime::new();
    let calls = Rc::new(Cell::new(0));
    let weak = Rc::downgrade(&calls);
    let captured = Rc::clone(&calls);
    let timer = RepeatingTimer::new(
        runtime.handle(),
        ms(1000),
        Rc::new(move || captured.set(captured.get() + 1)),
    );
    assert_eq!(calls.get(), 0);
    for (time, expected) in [(999, 0), (1000, 1), (1999, 1), (2000, 2)] {
        for result in runtime.advance_to(ms(time)) {
            result.unwrap();
        }
        assert_eq!(calls.get(), expected);
        assert_eq!(runtime.pending(), 1);
    }
    drop(calls);
    assert!(weak.upgrade().is_some());
    drop(timer);
    assert!(weak.upgrade().is_none());
    assert_eq!(runtime.pending(), 0);
    assert!(runtime.advance_to(ms(3000)).is_empty());
}

#[test]
fn repeating_timer_allows_callback_to_stop_or_replace_it() {
    use std::cell::RefCell;
    for replace in [false, true] {
        let runtime = ManualRuntime::new();
        let slot = Rc::new(RefCell::new(None));
        let weak_slot = Rc::downgrade(&slot);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let log = Rc::clone(&calls);
        let host = runtime.clone();
        *slot.borrow_mut() = Some(RepeatingTimer::new(
            runtime.handle(),
            ms(1000),
            Rc::new(move || {
                log.borrow_mut().push("old");
                let slot = weak_slot.upgrade().unwrap();
                let old = slot.take();
                drop(old);
                if replace {
                    let next_log = Rc::clone(&log);
                    let next = RepeatingTimer::new(
                        host.handle(),
                        ms(250),
                        Rc::new(move || next_log.borrow_mut().push("new")),
                    );
                    slot.replace(Some(next));
                }
            }),
        ));
        for result in runtime.advance_to(ms(1000)) {
            result.unwrap();
        }
        assert_eq!(*calls.borrow(), ["old"]);
        assert_eq!(runtime.next_deadline(), replace.then(|| ms(1250)));
        for result in runtime.advance_to(ms(1250)) {
            result.unwrap();
        }
        assert_eq!(
            *calls.borrow(),
            if replace {
                vec!["old", "new"]
            } else {
                vec!["old"]
            }
        );
        drop(slot.take());
        assert_eq!(runtime.pending(), 0);
        for result in runtime.advance_to(ms(2000)) {
            result.unwrap();
        }
        assert_eq!(
            *calls.borrow(),
            if replace {
                vec!["old", "new"]
            } else {
                vec!["old"]
            }
        );
    }
}
