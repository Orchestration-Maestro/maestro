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

/// Named observation phases of two overlapping, first-completes-first calls.
#[derive(Clone, Copy)]
pub enum Phase {
    /// Both calls reached their unopened host barriers.
    BothPending,
    /// First call completed while the second barrier remains unopened.
    FirstCompleted,
}

/// Runs two calls with explicit entry/completion witnesses and awaits both before returning.
pub async fn ordered<A, B, F, O>(
    first: A,
    second: B,
    barriers: [(
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ); 2],
    mut observe: O,
) -> (A::Output, B::Output)
where
    A: Future,
    B: Future,
    O: FnMut(Phase) -> F,
    F: Future<Output = ()>,
{
    let [(entered_first, open_first), (entered_second, open_second)] = barriers;
    let (first_done, first_finished) = tokio::sync::oneshot::channel();
    let (second_done, second_finished) = tokio::sync::oneshot::channel();
    let main = async {
        let first = first.await;
        first_done.send(()).unwrap();
        (first, second_finished.await.unwrap())
    };
    let helper = async {
        entered_first.await.unwrap();
        let control = async {
            entered_second.await.unwrap();
            observe(Phase::BothPending).await;
            open_first.send(()).unwrap();
            first_finished.await.unwrap();
            observe(Phase::FirstCompleted).await;
            open_second.send(()).unwrap();
        };
        let second = alongside(second, control).await;
        assert!(second_done.send(second).is_ok());
    };
    alongside(main, helper).await
}

/// Completes the second admitted call before releasing the first call's named gate.
pub async fn reversed<A, B, F, O>(
    first: A,
    second: B,
    barriers: [(
        tokio::sync::oneshot::Receiver<()>,
        tokio::sync::oneshot::Sender<()>,
    ); 2],
    mut observe: O,
) -> (A::Output, B::Output)
where
    A: Future,
    B: Future,
    O: FnMut(bool) -> F,
    F: Future<Output = ()>,
{
    let [(entered_first, open_first), (entered_second, open_second)] = barriers;
    let (done, finished) = tokio::sync::oneshot::channel();
    let main = async { (first.await, finished.await.unwrap()) };
    let helper = async {
        entered_first.await.unwrap();
        let control = async {
            entered_second.await.unwrap();
            observe(false).await;
            open_second.send(()).unwrap();
        };
        let second = alongside(second, control).await;
        observe(true).await;
        open_first.send(()).unwrap();
        assert!(done.send(second).is_ok());
    };
    alongside(main, helper).await
}
