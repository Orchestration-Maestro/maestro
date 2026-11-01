//! Controlled native waits and independent event tails shared by custom-view scripts.
use super::{
    host::{Imports, State},
    host_family::guest::Guest,
};
use std::{
    collections::VecDeque,
    future::{Future, poll_fn},
    pin::pin,
    sync::{Arc, Mutex},
    task::{Poll, Waker},
};
use tokio::sync::oneshot;
use wasmtime::component::Accessor;

/// A clock whose next pending native wait advances only when the script asks.
#[derive(Clone, Default)]
pub struct Clock(Arc<Mutex<Pending>>);
/// Waiting native operations and the script waiting to observe one.
#[derive(Default)]
struct Pending {
    /// One completion sender per admitted native wait.
    waits: VecDeque<oneshot::Sender<()>>,
    /// Wakes the script when a tail reaches its next wait.
    observer: Option<Waker>,
}
impl Clock {
    /// Suspends the original native wait without using elapsed wall time.
    pub async fn wait(&self) {
        let (open, opened) = oneshot::channel();
        {
            let mut pending = self.0.lock().unwrap();
            pending.waits.push_back(open);
            if let Some(waker) = pending.observer.take() {
                waker.wake();
            }
        }
        let _ = opened.await;
    }
    /// Waits until the independent tail has reached its next native wait.
    async fn settled(&self) {
        poll_fn(|cx| {
            let mut pending = self.0.lock().unwrap();
            pending.waits.retain(|wait| !wait.is_closed());
            if pending.waits.is_empty() {
                pending.observer = Some(cx.waker().clone());
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await;
    }
    /// Verifies closing the view dropped every pending native wait.
    pub fn assert_idle(&self) {
        assert!(
            self.0
                .lock()
                .unwrap()
                .waits
                .iter()
                .all(oneshot::Sender::is_closed)
        );
    }
    /// Completes exactly the requested waits and observes the next suspended tick.
    pub async fn advance(&self, ticks: usize) {
        for _ in 0..ticks {
            self.settled().await;
            self.0
                .lock()
                .unwrap()
                .waits
                .pop_front()
                .unwrap()
                .send(())
                .unwrap();
        }
        if ticks > 0 {
            self.settled().await;
        }
    }
}
/// Polls announced guest tails beside a scripted view and waits for their cleanup on close.
pub async fn drive<T>(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    work: impl Future<Output = wasmtime::Result<Result<T, String>>>,
) -> wasmtime::Result<Result<T, String>> {
    let mut work = pin!(work);
    let mut result = None;
    let mut tails = Vec::new();
    poll_fn(|cx| {
        if result.is_none()
            && let Poll::Ready(done) = work.as_mut().poll(cx)
        {
            result = Some(done);
        }
        let announced =
            accessor.with(|mut access| std::mem::take(&mut access.get().observed.bus.unreleased));
        for id in announced {
            tails.push(Box::pin(tail(accessor, guest, id)));
        }
        let mut index = 0;
        while index < tails.len() {
            match tails[index].as_mut().poll(cx) {
                Poll::Pending => index += 1,
                Poll::Ready(done) => {
                    drop(tails.swap_remove(index));
                    done?.map_err(wasmtime::Error::msg)?;
                }
            }
        }
        if tails.is_empty()
            && let Some(result) = result.take()
        {
            Poll::Ready(result)
        } else {
            Poll::Pending
        }
    })
    .await
}

/// Consumes one announced tail and releases its guest-owned identity exactly once.
async fn tail(
    accessor: &Accessor<State, Imports>,
    guest: &Guest,
    id: u32,
) -> wasmtime::Result<Result<(), String>> {
    let answer = guest
        .call_invoke_tail(accessor, super::host::borrow(id))
        .await?;
    let identity = accessor.with(|mut access| {
        access
            .get()
            .table
            .get(&super::host::borrow(id))
            .map(|callback| callback.0)
    })?;
    super::custom_host::release_retained(accessor, guest, &super::host::borrow(id)).await?;
    accessor.with(|mut access| {
        assert_eq!(
            access
                .get()
                .observed
                .dropped
                .iter()
                .filter(|dropped| **dropped == identity)
                .count(),
            1
        );
    });
    Ok::<_, wasmtime::Error>(answer)
}
