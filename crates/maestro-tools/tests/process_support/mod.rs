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

/// Drives `future` to completion with a waker that [`Forward`] owns, drops the
/// future, and returns its output with that owner: a count of one means nothing the
/// future registered still holds a waker.
pub async fn polled_to_completion<F: Future>(future: F) -> (F::Output, Arc<Forward>) {
    let forward = Arc::new(Forward(Mutex::new(None)));
    let mut future = Box::pin(future);
    let output = std::future::poll_fn(|cx| {
        *forward.0.lock().unwrap() = Some(cx.waker().clone());
        let waker = Waker::from(Arc::clone(&forward));
        future.as_mut().poll(&mut Context::from_waker(&waker))
    })
    .await;
    drop(future);
    *forward.0.lock().unwrap() = None;
    (output, forward)
}
