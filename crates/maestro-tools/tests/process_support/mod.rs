//! Polling helpers shared by the process tests.
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll, Wake, Waker};

/// Polls `future` once and reports its state without consuming it.
pub async fn poll_once<F: Future>(future: &mut Pin<&mut F>) -> Poll<F::Output> {
    std::future::poll_fn(|cx| Poll::Ready(future.as_mut().poll(cx))).await
}

/// Forwards wake-ups to the polling task while counting as one owner of itself.
pub struct Forward(Mutex<Option<Waker>>);

impl Wake for Forward {
    fn wake(self: Arc<Self>) {
        if let Some(waker) = self.0.lock().unwrap().as_ref() {
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
    *forward.0.lock().unwrap() = Some(cx.waker().clone());
    let waker = Waker::from(Arc::clone(forward));
    future.as_mut().poll(&mut Context::from_waker(&waker))
}

/// Drives `future` to completion through `forward`, drops the future, and clears the
/// stored task waker: a count of one means nothing the future registered still holds a
/// waker.
pub async fn complete<F: Future>(forward: &Arc<Forward>, mut future: Pin<Box<F>>) -> F::Output {
    let output = std::future::poll_fn(|cx| poll_through(forward, &mut future, cx)).await;
    drop(future);
    *forward.0.lock().unwrap() = None;
    output
}

/// Drives `future` to completion with a fresh [`forward`] owner and returns its output
/// with that owner.
pub async fn polled_to_completion<F: Future>(future: F) -> (F::Output, Arc<Forward>) {
    let owner = forward();
    let output = complete(&owner, Box::pin(future)).await;
    (output, owner)
}
