//! Polls a helper alongside a future on one task.
use std::future::{Future, poll_fn};
use std::pin::pin;

/// Resolves with the main future's output. While the main future is pending, the helper is
/// polled after it; the helper is dropped unfinished when the main future needs nothing more
/// from it.
pub async fn alongside<M: Future, H: Future>(main: M, helper: H) -> M::Output {
    let (mut main, mut helper) = (pin!(main), pin!(helper));
    let mut helper_done = false;
    poll_fn(|cx| {
        let state = main.as_mut().poll(cx);
        if state.is_pending() && !helper_done {
            helper_done = helper.as_mut().poll(cx).is_ready();
        }
        state
    })
    .await
}
