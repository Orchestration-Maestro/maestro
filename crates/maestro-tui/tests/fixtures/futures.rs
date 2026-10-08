//! Minimal future helpers for driving asynchronous contracts without a runtime.

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

/// Runs a future to completion with a waker that does nothing.
pub fn block_on<T>(future: impl Future<Output = T>) -> T {
    let mut future = Box::pin(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

/// Resolves after being polled once, to exercise suspended work.
pub struct YieldOnce(pub bool);

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            return Poll::Ready(());
        }
        self.0 = true;
        context.waker().wake_by_ref();
        Poll::Pending
    }
}
