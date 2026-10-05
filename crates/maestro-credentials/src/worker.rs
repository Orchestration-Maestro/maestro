//! Owned blocking work with request-local wakeable waiting.
use crate::CredentialError;
use maestro_models::Cancellation;
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
    thread::JoinHandle,
};

struct State<T> {
    result: Option<Result<T, CredentialError>>,
    waiters: Vec<(Arc<()>, Waker)>,
}
pub(crate) struct Work<T> {
    state: Arc<Mutex<State<T>>>,
    handle: Option<JoinHandle<()>>,
}
impl<T: Clone + Send + 'static> Work<T> {
    pub(crate) fn start(
        run: impl FnOnce() -> Result<T, CredentialError> + Send + 'static,
    ) -> Arc<Self> {
        let state = Arc::new(Mutex::new(State {
            result: None,
            waiters: vec![],
        }));
        let owned = state.clone();
        let handle = std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
                .unwrap_or(Err(CredentialError::Storage));
            let waiters = {
                let mut state = owned.lock().unwrap_or_else(|p| p.into_inner());
                state.result = Some(result);
                std::mem::take(&mut state.waiters)
            };
            for (_, waker) in waiters {
                waker.wake();
            }
        });
        Arc::new(Self {
            state,
            handle: Some(handle),
        })
    }
    pub(crate) async fn wait(&self, cancellation: Cancellation) -> Result<T, CredentialError> {
        let mut waiter = Waiter {
            work: self,
            id: Arc::new(()),
        };
        cancellable(&cancellation, poll_fn(|cx| waiter.poll(cx))).await
    }
}
impl<T> Drop for Work<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            // Reaping remains owned without blocking a dropped resolver future.
            std::thread::spawn(move || {
                let _ = handle.join();
            });
        }
    }
}
struct Waiter<'a, T> {
    work: &'a Work<T>,
    id: Arc<()>,
}
impl<T: Clone> Waiter<'_, T> {
    fn poll(&mut self, cx: &mut Context<'_>) -> Poll<Result<T, CredentialError>> {
        let new = cx.waker().clone();
        let mut retired = None;
        let mut state = self.work.state.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(result) = &state.result {
            return Poll::Ready(result.clone());
        }
        if let Some((_, old)) = state
            .waiters
            .iter_mut()
            .find(|(id, _)| Arc::ptr_eq(id, &self.id))
        {
            if !old.will_wake(&new) {
                retired = Some(std::mem::replace(old, new));
            }
        } else {
            state.waiters.push((self.id.clone(), new));
        }
        drop(state);
        drop(retired);
        Poll::Pending
    }
}
impl<T> Drop for Waiter<'_, T> {
    fn drop(&mut self) {
        let retired = {
            let mut state = self.work.state.lock().unwrap_or_else(|p| p.into_inner());
            state
                .waiters
                .iter()
                .position(|(id, _)| Arc::ptr_eq(id, &self.id))
                .map(|i| state.waiters.swap_remove(i))
        };
        drop(retired);
    }
}
pub(crate) async fn cancellable<T>(
    cancellation: &Cancellation,
    future: impl Future<Output = Result<T, CredentialError>>,
) -> Result<T, CredentialError> {
    let mut cancelled = std::pin::pin!(cancellation.cancelled());
    let mut future = std::pin::pin!(future);
    poll_fn(|cx| {
        if cancelled.as_mut().poll(cx).is_ready() {
            return Poll::Ready(Err(CredentialError::Cancelled));
        }
        match Pin::as_mut(&mut future).poll(cx) {
            Poll::Ready(_) if cancellation.is_cancelled() => {
                Poll::Ready(Err(CredentialError::Cancelled))
            }
            result => result,
        }
    })
    .await
}
