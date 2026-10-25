//! Scoped subscribers keep their events whichever thread reaches a callsite first.
#![cfg(test)]

use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Waker},
};
use tracing::{Event, Metadata, span};

/// Counts the events it receives.
#[derive(Clone, Default)]
struct Count(Arc<AtomicUsize>);

impl tracing::Subscriber for Count {
    fn enabled(&self, _: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &span::Attributes<'_>) -> span::Id {
        span::Id::from_u64(1)
    }
    fn record(&self, _: &span::Id, _: &span::Record<'_>) {}
    fn record_follows_from(&self, _: &span::Id, _: &span::Id) {}
    fn event(&self, _: &Event<'_>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn enter(&self, _: &span::Id) {}
    fn exit(&self, _: &span::Id) {}
}

/// The one callsite both threads reach.
fn reach() {
    tracing::warn!("reached");
}

/// Reach the callsite on a thread without a subscriber, then on this one.
fn reach_after_another_thread() {
    std::thread::spawn(reach).join().unwrap();
    reach();
}

#[test]
fn events_arrive_when_another_thread_reached_the_callsite_first() {
    let count = Count::default();
    maestro_test_tracing::with_default(count.clone(), reach_after_another_thread);
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
}

#[test]
fn future_events_arrive_when_another_thread_reached_the_callsite_first() {
    let count = Count::default();
    let future = maestro_test_tracing::with_subscriber(
        async { reach_after_another_thread() },
        count.clone(),
    );
    assert!(
        std::pin::pin!(future)
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_ready()
    );
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
}
