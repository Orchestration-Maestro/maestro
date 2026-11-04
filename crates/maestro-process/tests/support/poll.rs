//! One-shot polling shared by the unit and integration test programs.
#![cfg(test)]

use std::future::Future;
use std::pin::Pin;
use std::task::Poll;

/// Polls `future` once and reports whether it is still pending.
pub(crate) async fn is_pending<F: Future + Unpin>(future: &mut F) -> bool {
    std::future::poll_fn(|cx| Poll::Ready(Pin::new(&mut *future).poll(cx).is_pending())).await
}
