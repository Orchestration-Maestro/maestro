//! Wakeable request-local cancellation without a runtime or timer.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

#[derive(Default)]
struct State {
    cancelled: bool,
    waiters: Vec<(Arc<()>, Waker)>,
}

/// A fresh signal whose clones share local request cancellation.
#[derive(Clone, Default)]
pub struct Cancellation {
    state: Arc<Mutex<State>>,
}

impl Cancellation {
    /// Create an independent, uncancelled signal.
    pub fn new() -> Self {
        Self::default()
    }
    /// Idempotently cancel this request, waking all waiters outside the lock.
    pub fn cancel(&self) {
        let waiters = {
            let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
            if state.cancelled {
                return;
            }
            state.cancelled = true;
            std::mem::take(&mut state.waiters)
        };
        for (_, waker) in waiters {
            waker.wake();
        }
    }
    /// Read whether this request has been cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .cancelled
    }
    /// Await cancellation, including cancellation before polling.
    /// Dropping this future unregisters its waiter without cancelling the request.
    pub async fn cancelled(&self) {
        Waiter {
            cancellation: self,
            id: None,
        }
        .await;
    }
}

struct Waiter<'a> {
    cancellation: &'a Cancellation,
    id: Option<Arc<()>>,
}
impl Future for Waiter<'_> {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let cancellation = self.cancellation;
        let new_waker = cx.waker().clone();
        let mut retired = None;
        let mut state = cancellation.state.lock().unwrap_or_else(|p| p.into_inner());
        if state.cancelled {
            return Poll::Ready(());
        }
        if let Some(id) = &self.id {
            if let Some((_, waker)) = state
                .waiters
                .iter_mut()
                .find(|(key, _)| Arc::ptr_eq(key, id))
                && !waker.will_wake(&new_waker)
            {
                retired = Some(std::mem::replace(waker, new_waker));
            }
        } else {
            let id = Arc::new(());
            state.waiters.push((id.clone(), new_waker));
            self.id = Some(id);
        }
        drop(state);
        // Waker callbacks may reenter the signal, including when a waker drops.
        drop(retired);
        Poll::Pending
    }
}
impl Drop for Waiter<'_> {
    fn drop(&mut self) {
        if let Some(id) = &self.id {
            let retired = {
                let mut state = self
                    .cancellation
                    .state
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                state
                    .waiters
                    .iter()
                    .position(|(key, _)| Arc::ptr_eq(key, id))
                    .map(|position| state.waiters.swap_remove(position))
            };
            drop(retired);
        }
    }
}

impl serde::Serialize for Cancellation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        serializer.serialize_map(Some(0))?.end()
    }
}
impl<'de> serde::Deserialize<'de> for Cancellation {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let _ = serde_json::Value::deserialize(deserializer)?;
        Ok(Self::new())
    }
}
