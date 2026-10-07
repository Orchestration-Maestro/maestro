mod support;
use maestro_models::*;
use std::sync::{Arc, atomic::Ordering};
use support::{WakeCounter, poll};

#[test]
fn cancellation_waiters_are_idempotent_and_request_local() {
    let cancellation = Cancellation::new();
    let clone = cancellation.clone();
    let first = Arc::new(WakeCounter::default());
    let second = Arc::new(WakeCounter::default());
    let stale = Arc::new(WakeCounter::default());
    let dropped = Arc::new(WakeCounter::default());
    let mut one = Box::pin(cancellation.cancelled());
    let mut two = Box::pin(clone.cancelled());
    assert!(poll(one.as_mut(), &stale).is_pending());
    assert!(poll(one.as_mut(), &first).is_pending());
    assert!(poll(one.as_mut(), &first).is_pending());
    assert!(poll(two.as_mut(), &second).is_pending());
    {
        let mut abandoned = Box::pin(cancellation.cancelled());
        assert!(poll(abandoned.as_mut(), &dropped).is_pending());
    }
    clone.cancel();
    clone.cancel();
    cancellation.cancel();
    assert_eq!(first.0.load(Ordering::SeqCst), 1);
    assert_eq!(second.0.load(Ordering::SeqCst), 1);
    assert_eq!(stale.0.load(Ordering::SeqCst), 0);
    assert_eq!(dropped.0.load(Ordering::SeqCst), 0);
    assert!(poll(one.as_mut(), &first).is_ready());
    assert!(poll(two.as_mut(), &second).is_ready());
    let mut after = Box::pin(cancellation.cancelled());
    assert!(poll(after.as_mut(), &first).is_ready());
    assert!(cancellation.is_cancelled());
    let options = StreamOptions {
        signal: Some(Cancellation::new()),
        ..Default::default()
    };
    let independent = StreamOptions {
        signal: Some(Cancellation::new()),
        ..Default::default()
    };
    let shared = options.clone();
    shared.signal.as_ref().unwrap().cancel();
    assert!(options.signal.as_ref().unwrap().is_cancelled());
    assert!(!independent.signal.as_ref().unwrap().is_cancelled());
    assert!(!Cancellation::default().is_cancelled());
}
