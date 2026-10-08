//! Polls two futures together on one task.
use std::future::{Future, poll_fn};
use std::pin::pin;
use std::task::Poll;

/// Resolves when both futures have resolved, polling them in turn.
pub async fn both<A: Future, B: Future>(first: A, second: B) -> (A::Output, B::Output) {
    let (mut first, mut second) = (pin!(first), pin!(second));
    let (mut first_done, mut second_done) = (None, None);
    poll_fn(|cx| {
        if first_done.is_none()
            && let Poll::Ready(value) = first.as_mut().poll(cx)
        {
            first_done = Some(value);
        }
        if second_done.is_none()
            && let Poll::Ready(value) = second.as_mut().poll(cx)
        {
            second_done = Some(value);
        }
        match (first_done.take(), second_done.take()) {
            (Some(a), Some(b)) => Poll::Ready((a, b)),
            (a, b) => {
                first_done = a;
                second_done = b;
                Poll::Pending
            }
        }
    })
    .await
}
