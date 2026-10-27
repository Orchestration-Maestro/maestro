//! One cancellable startup application, scheduled by the existing event bus.
use maestro_extensions_wasm::{ExtensionAPI, ExtensionFuture, ExtensionResult};
use std::{
    cell::RefCell,
    future::poll_fn,
    rc::Rc,
    task::{Poll, Waker},
};

/// The factory owns at most one pending startup application.
pub(super) type Slot = Rc<RefCell<Option<Rc<RefCell<Startup>>>>>;

/// Work the listener tail owns until completion or session replacement.
#[derive(Default)]
pub(super) struct Startup {
    /// Awaited application, taken before polling to allow host reentry.
    future: Option<ExtensionFuture<'static, ()>>,
    /// Tail awaiting a wake from cancellation or the selected host operation.
    waker: Option<Waker>,
    /// Cancellation wins even if the host reenters during a poll.
    cancelled: bool,
}

/// Releases old work before another session or command changes the preset.
pub(super) fn cancel(slot: &RefCell<Option<Rc<RefCell<Startup>>>>) {
    let previous = slot.borrow_mut().take();
    if let Some(work) = previous {
        let (future, waker) = {
            let mut work = work.borrow_mut();
            work.cancelled = true;
            (work.future.take(), work.waker.take())
        };
        drop(future);
        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

/// Installs a listener whose synchronous prefix hands off an independently owned tail.
pub(super) fn register(api: &ExtensionAPI, slot: Slot) -> ExtensionResult<()> {
    api.events().on(
        "preset:startup",
        Rc::new(move |_, _| {
            let Some(work) = slot.borrow().clone() else {
                return Ok(None);
            };
            Ok(Some(Box::pin(poll_fn(move |cx| {
                let future = work.borrow_mut().future.take();
                let Some(mut future) = future else {
                    return Poll::Ready(Ok(()));
                };
                if work.borrow().cancelled {
                    return Poll::Ready(Ok(()));
                }
                let answer = future.as_mut().poll(cx);
                let mut state = work.borrow_mut();
                if state.cancelled {
                    return Poll::Ready(Ok(()));
                }
                state.waker = None;
                if answer.is_pending() {
                    state.future = Some(future);
                    state.waker = Some(cx.waker().clone());
                }
                answer
            }))))
        }),
    )?;
    Ok(())
}

/// Places one future in the slot; emitting the channel admits it without waiting for it.
pub(super) fn set(
    slot: &RefCell<Option<Rc<RefCell<Startup>>>>,
    future: ExtensionFuture<'static, ()>,
) {
    cancel(slot);
    *slot.borrow_mut() = Some(Rc::new(RefCell::new(Startup {
        future: Some(future),
        ..Startup::default()
    })));
}
