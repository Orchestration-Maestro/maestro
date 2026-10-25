//! Polling and pipe ownership helpers shared by the process tests.
#![cfg(test)]
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

/// Polls `future` once and reports its state without consuming it.
#[cfg(not(target_arch = "wasm32"))]
pub async fn poll_once<F: Future>(future: &mut Pin<&mut F>) -> Poll<F::Output> {
    std::future::poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await
}

/// Forwards wake-ups outside its mutex while counting as one owner of itself.
pub struct Forward(Mutex<Option<Arc<Waker>>>);

impl Wake for Forward {
    fn wake(self: Arc<Self>) {
        let waker = self.0.lock().unwrap().clone();
        if let Some(waker) = waker {
            waker.wake_by_ref();
        }
    }
}

/// A fresh [`Forward`] that no waker has been stored in yet.
pub fn forward() -> Arc<Forward> {
    Arc::new(Forward(Mutex::new(None)))
}

/// Polls `future` once with a waker owned by `forward`, so wake-ups it registers reach
/// the task polling through `cx`.
pub fn poll_through<F: Future>(
    forward: &Arc<Forward>,
    future: &mut Pin<Box<F>>,
    cx: &Context<'_>,
) -> Poll<F::Output> {
    let caller = Arc::new(cx.waker().clone());
    let previous = forward.0.lock().unwrap().replace(caller);
    drop(previous);
    let waker = Waker::from(Arc::clone(forward));
    future.as_mut().poll(&mut Context::from_waker(&waker))
}

/// Drives `future` to completion through `forward`, drops the future, and clears the
/// stored task waker: a count of one means nothing the future registered still holds a
/// waker.
pub async fn complete<F: Future>(forward: &Arc<Forward>, mut future: Pin<Box<F>>) -> F::Output {
    let output = std::future::poll_fn(|cx| poll_through(forward, &mut future, cx)).await;
    drop(future);
    let stored = forward.0.lock().unwrap().take();
    drop(stored);
    output
}

/// Drives `future` to completion with a fresh [`forward`] owner and returns its output
/// with that owner.
#[cfg(not(target_arch = "wasm32"))]
pub async fn polled_to_completion<F: Future>(future: F) -> (F::Output, Arc<Forward>) {
    let owner = forward();
    let output = complete(&owner, Box::pin(future)).await;
    (output, owner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Weak;

    /// Callback observations in invocation order.
    type Events = Arc<Mutex<Vec<(&'static str, bool, bool)>>>;

    /// Records guard availability without retaining the forwarding owner.
    struct Probe {
        /// Non-owning callback target.
        owner: Weak<Forward>,
        /// Shared observations retained by the test.
        events: Events,
    }

    /// Records slot availability without calling caller code under the guard.
    fn record(owner: &Weak<Forward>, events: &Events, kind: &'static str) {
        let owner = owner.upgrade().unwrap();
        let (unlocked, empty) = match owner.0.try_lock() {
            Ok(slot) => (true, slot.is_none()),
            Err(_) => (false, false),
        };
        events.lock().unwrap().push((kind, unlocked, empty));
    }

    impl Wake for Probe {
        fn wake(self: Arc<Self>) {
            record(&self.owner, &self.events, "wake");
        }

        fn wake_by_ref(self: &Arc<Self>) {
            record(&self.owner, &self.events, "wake");
        }
    }

    impl Drop for Probe {
        fn drop(&mut self) {
            record(&self.owner, &self.events, "drop");
        }
    }

    /// Creates a safe caller waker whose final destruction is observable.
    fn probe(owner: &Arc<Forward>, events: &Events) -> Waker {
        Waker::from(Arc::new(Probe {
            owner: Arc::downgrade(owner),
            events: Arc::clone(events),
        }))
    }

    /// Leaves the last caller-waker owner in the slot before returning ready.
    struct Completion {
        /// Non-owning forwarding slot.
        owner: Weak<Forward>,
        /// Ordered destruction observations.
        events: Events,
    }

    impl Future for Completion {
        type Output = i32;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            let owner = self.owner.upgrade().unwrap();
            let waker = probe(&owner, &self.events);
            let mut inner = Box::pin(std::future::ready(37));
            poll_through(&owner, &mut inner, &Context::from_waker(&waker))
        }
    }

    impl Drop for Completion {
        fn drop(&mut self) {
            record(&self.owner, &self.events, "future");
        }
    }

    #[test]
    fn forward_completion_drops_after_unlock() {
        let owner = forward();
        let events = Events::default();
        let state = Completion {
            owner: Arc::downgrade(&owner),
            events: Arc::clone(&events),
        };
        let mut completion = Box::pin(complete(&owner, Box::pin(state)));
        assert_eq!(
            completion
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop())),
            Poll::Ready(37)
        );
        drop(completion);
        assert_eq!(Arc::strong_count(&owner), 1);
        assert!(owner.0.lock().unwrap().is_none());
        assert_eq!(
            events.lock().unwrap().as_slice(),
            [("future", true, false), ("drop", true, true)]
        );
    }

    #[test]
    fn forward_replacement_drops_after_unlock() {
        let owner = forward();
        let events = Events::default();
        let waker = probe(&owner, &events);
        let mut future = Box::pin(std::future::pending::<()>());
        assert!(poll_through(&owner, &mut future, &Context::from_waker(&waker)).is_pending());
        drop(waker);
        let replacement = Waker::noop();
        assert!(poll_through(&owner, &mut future, &Context::from_waker(replacement)).is_pending());
        assert_eq!(events.lock().unwrap().as_slice(), [("drop", true, false)]);
        drop(future);
        let stored = owner.0.lock().unwrap().take();
        drop(stored);
        assert_eq!(Arc::strong_count(&owner), 1);
    }

    #[test]
    fn forward_wakes_after_unlock() {
        let owner = forward();
        let events = Events::default();
        Arc::clone(&owner).wake();
        assert!(events.lock().unwrap().is_empty());
        let waker = probe(&owner, &events);
        let mut future = Box::pin(std::future::pending::<()>());
        assert!(poll_through(&owner, &mut future, &Context::from_waker(&waker)).is_pending());
        Arc::clone(&owner).wake();
        Arc::clone(&owner).wake();
        let observed = events.lock().unwrap().clone();
        drop(future);
        drop(waker);
        let stored = owner.0.lock().unwrap().take();
        drop(stored);
        assert_eq!(Arc::strong_count(&owner), 1);
        assert_eq!(observed, [("wake", true, false), ("wake", true, false)]);
    }
}
