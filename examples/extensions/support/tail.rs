//! Cancellable work handed to the host's independent event-tail executor.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{EventListener, ExtensionFuture, ExtensionResult};
use std::{
    cell::RefCell,
    future::poll_fn,
    rc::Rc,
    task::{Context, Poll, Waker},
};

/// One future and its host task, including cancellation during a synchronous callback.
struct Work {
    /// Dropping the future immediately releases its native wait or request.
    running: Option<ExtensionFuture<'static, ()>>,
    /// Cleared on cancellation even when the future is temporarily out for polling.
    waker: Option<Waker>,
    /// Prevents another copy's channel emission from claiming the same work twice.
    claimed: bool,
}

/// Shared ownership keeps a claimed future alive until its host tail settles.
type Current = Rc<RefCell<Work>>;

/// An independently cancellable slot shared by lifecycle handlers and an event listener.
#[derive(Clone, Default)]
pub struct Tail(Rc<RefCell<Option<Current>>>);

impl Tail {
    /// Replaces the current work, synchronously releasing the previous future.
    pub fn start(&self, running: ExtensionFuture<'static, ()>) {
        self.stop();
        self.0.replace(Some(Rc::new(RefCell::new(Work {
            running: Some(running),
            waker: None,
            claimed: false,
        }))));
    }

    /// Releases pending native operations and wakes the host to retire its tail.
    pub fn stop(&self) {
        let Some(work) = self.0.borrow_mut().take() else {
            return;
        };
        let mut work = work.borrow_mut();
        work.running = None;
        if let Some(waker) = work.waker.take() {
            waker.wake();
        }
    }

    /// Hands each installed future to the host once, without retaining this slot in the bus.
    #[must_use]
    pub fn listener(&self) -> EventListener {
        let weak = Rc::downgrade(&self.0);
        Rc::new(move |_, _| {
            let Some(owner) = weak.upgrade() else {
                return Ok(None);
            };
            let Some(work) = owner.borrow().clone() else {
                return Ok(None);
            };
            if std::mem::replace(&mut work.borrow_mut().claimed, true) {
                return Ok(None);
            }
            Ok(Some(Box::pin(poll_fn(move |cx| poll_work(&work, cx)))))
        })
    }
}

/// Polls outside the slot borrow so synchronous view disposal can cancel the same work.
fn poll_work(work: &Current, cx: &mut Context<'_>) -> Poll<ExtensionResult<()>> {
    let running = {
        let mut work = work.borrow_mut();
        work.waker = Some(cx.waker().clone());
        work.running.take()
    };
    let Some(mut running) = running else {
        work.borrow_mut().waker = None;
        return Poll::Ready(Ok(()));
    };
    // A completion callback may synchronously dispose the view that owns this work.
    let result = running.as_mut().poll(cx);
    let mut work = work.borrow_mut();
    if result.is_pending() && work.waker.is_some() {
        work.running = Some(running);
        Poll::Pending
    } else {
        work.waker = None;
        match result {
            Poll::Ready(result) => Poll::Ready(result),
            Poll::Pending => Poll::Ready(Ok(())),
        }
    }
}
