//! Timer ownership; native dispatch is browser-qualified.
#![cfg(test)]
use crate::effects_support;
use effects_support::Host;
use maestro_web::browser::attachment_content::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[test]
fn browser_timeout_delivers_once_at_the_supplied_deadline() {
    let host = Host::default();
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    let _timer = host
        .timer(5000, false, Rc::new(move || seen.set(seen.get() + 1)))
        .unwrap();
    host.advance(4999);
    assert_eq!(count.get(), 0);
    host.advance(5000);
    assert_eq!(count.get(), 1);
    host.advance(15000);
    assert_eq!(count.get(), 1);
}

#[test]
fn browser_polling_reuses_one_interval_owner() {
    let host = Host::default();
    let count = Rc::new(Cell::new(0));
    let seen = count.clone();
    let _timer = host
        .timer(500, true, Rc::new(move || seen.set(seen.get() + 1)))
        .unwrap();
    for (instant, expected) in [(499, 0), (500, 1), (1000, 2), (1500, 3)] {
        host.advance(instant);
        assert_eq!(count.get(), expected);
    }
    assert_eq!(host.admissions.get(), 1);
    assert_eq!(host.live(), 1);
}

#[test]
fn browser_timer_drop_revokes_the_callback() {
    for repeat in [false, true] {
        for cancel_first in [false, true] {
            let host = Host::default();
            let captured = Rc::new(Cell::new(0));
            let weak = Rc::downgrade(&captured);
            let seen = captured.clone();
            let mut timer = host
                .timer(500, repeat, Rc::new(move || seen.set(seen.get() + 1)))
                .unwrap();
            let neighbor = Rc::new(Cell::new(0));
            let seen = neighbor.clone();
            let _live = host
                .timer(500, false, Rc::new(move || seen.set(seen.get() + 1)))
                .unwrap();
            if cancel_first {
                timer.cancel();
                timer.cancel();
            }
            drop(timer);
            drop(captured);
            assert!(weak.upgrade().is_none());
            host.advance(1500);
            assert_eq!(neighbor.get(), 1);
            assert_eq!(host.live(), 0);
        }
    }
}

#[test]
fn browser_effects_keep_local_futures_and_host_failures() {
    let host = Rc::new(Host::default());
    let completed = Rc::new(Cell::new(false));
    let seen = completed.clone();
    host.spawn_local(Box::pin(async move {
        std::future::ready(()).await;
        seen.set(true);
    }));
    assert!(!completed.get());
    *host.failure.borrow_mut() = Some(AttachmentHostError::BorrowConflict);
    assert!(matches!(
        host.timer(500, false, Rc::new(|| {})),
        Err(AttachmentHostError::BorrowConflict)
    ));
    host.run_work();
    assert!(completed.get());
    assert_eq!(host.admissions.get(), 0);
}

#[test]
fn browser_timer_owners_cancel_independently_during_delivery() {
    let host = Host::default();
    let handle = Rc::new(RefCell::new(None::<maestro_web::browser::timer::Timer>));
    let stop = handle.clone();
    let first = Rc::new(Cell::new(0));
    let seen = first.clone();
    *handle.borrow_mut() = Some(
        host.timer(
            500,
            true,
            Rc::new(move || {
                seen.set(seen.get() + 1);
                let timer = stop.borrow_mut().take();
                drop(timer);
            }),
        )
        .unwrap(),
    );
    let second = Rc::new(Cell::new(0));
    let seen = second.clone();
    let _neighbor = host
        .timer(700, true, Rc::new(move || seen.set(seen.get() + 1)))
        .unwrap();
    host.advance(2100);
    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 3);
    assert!(handle.borrow().is_none());
    assert_eq!(host.live(), 1);
}
