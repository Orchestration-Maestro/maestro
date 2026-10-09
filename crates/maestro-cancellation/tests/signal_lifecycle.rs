//! Shared cancellation state and owned observations.

use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::task::{Context, Poll, Wake, Waker};

use maestro_cancellation::Cancellation;

#[test]
fn cancellation_clones_share_latched_state() {
    let signal = Cancellation::new();
    let clone = signal.clone();
    let independent = Cancellation::default();
    assert!(!signal.is_aborted());
    assert!(!clone.is_aborted());
    assert!(!independent.is_aborted());
    clone.abort();
    assert!(signal.is_aborted());
    signal.abort();
    assert!(clone.is_aborted());
    assert!(!independent.is_aborted());
}

/// Counts notifications and re-enters the same signal synchronously.
struct Observer {
    /// Notifications delivered by the primitive.
    wakes: AtomicUsize,
    /// Shared signal read by the wake callback.
    signal: Cancellation,
}
impl Wake for Observer {
    fn wake(self: Arc<Self>) {
        assert!(self.signal.is_aborted());
        self.signal.abort();
        self.wakes.fetch_add(1, Ordering::SeqCst);
    }
}
/// Polls an owned observer using a named counting waker.
fn observe<F: Future<Output = ()>>(future: Pin<&mut F>, observer: &Arc<Observer>) -> Poll<()> {
    let waker = Waker::from(Arc::clone(observer));
    future.poll(&mut Context::from_waker(&waker))
}
#[test]
fn cancellation_notifies_pending_and_late_observers() {
    let signal = Cancellation::new();
    let observer = Arc::new(Observer {
        wakes: AtomicUsize::new(0),
        signal: signal.clone(),
    });
    let mut first = Box::pin(signal.cancelled());
    let mut second = Box::pin(signal.cancelled());
    let mut unpolled = Box::pin(signal.cancelled());
    assert_eq!(observe(first.as_mut(), &observer), Poll::Pending);
    assert_eq!(observe(second.as_mut(), &observer), Poll::Pending);
    signal.abort();
    assert_eq!(observer.wakes.load(Ordering::SeqCst), 2);
    signal.abort();
    assert_eq!(observer.wakes.load(Ordering::SeqCst), 2);
    assert_eq!(observe(first.as_mut(), &observer), Poll::Ready(()));
    assert_eq!(observe(second.as_mut(), &observer), Poll::Ready(()));
    assert_eq!(observe(unpolled.as_mut(), &observer), Poll::Ready(()));
    assert_eq!(
        observe(Box::pin(signal.cancelled()).as_mut(), &observer),
        Poll::Ready(())
    );
}

#[test]
fn cancellation_observation_survives_handle_and_waiter_drops() {
    let signal = Cancellation::new();
    let retained = signal.clone();
    let observer = Arc::new(Observer {
        wakes: AtomicUsize::new(0),
        signal: retained.clone(),
    });
    let mut discarded = Box::pin(signal.cancelled());
    let mut surviving = Box::pin(signal.cancelled());
    assert_eq!(observe(discarded.as_mut(), &observer), Poll::Pending);
    assert_eq!(observe(surviving.as_mut(), &observer), Poll::Pending);
    drop(discarded);
    drop(signal);
    assert_eq!(observe(surviving.as_mut(), &observer), Poll::Pending);
    retained.abort();
    assert_eq!(observer.wakes.load(Ordering::SeqCst), 1);
    assert_eq!(observe(surviving.as_mut(), &observer), Poll::Ready(()));
    let abandoned = Cancellation::new();
    let mut future = Box::pin(abandoned.cancelled());
    drop(abandoned);
    assert_eq!(observe(future.as_mut(), &observer), Poll::Pending);
}
