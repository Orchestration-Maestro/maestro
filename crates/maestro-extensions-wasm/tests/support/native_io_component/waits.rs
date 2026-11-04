//! A wait retained between commands observes its original caller's signal.
#![forbid(
    clippy::pedantic,
    clippy::too_many_arguments,
    clippy::excessive_nesting
)]
use maestro_extensions_wasm::{
    AbortListener, AbortSignal, ExtensionCommandContext, ExtensionFuture, ExtensionResult,
    SignalPort, Subscription, SubscriptionPort, io::NativeIo,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    task::Poll,
    time::Duration,
};

/// An independently owned wait and its caller's subscription-removal count.
pub(super) struct Pending {
    /// The same admitted future is polled and later awaited or dropped.
    work: ExtensionFuture<'static, ()>,
    /// Present only when the caller supplied a cancellation signal.
    removed: Option<Rc<Cell<u32>>>,
}
impl Pending {
    /// Completes the selected future and checks caller-side release even on failure.
    pub(super) async fn finish(mut self) -> ExtensionResult<()> {
        let result = self.work.as_mut().await;
        self.check()?;
        result
    }
    /// Checks that the original caller's subscription was removed exactly once.
    fn check(&self) -> ExtensionResult<()> {
        if self.removed.as_ref().is_some_and(|count| count.get() != 1) {
            return Err("wait kept its caller subscription".into());
        }
        Ok(())
    }
}

/// Wraps the caller signal solely to observe its explicit remover's effect.
struct Tracked(AbortSignal, Rc<Cell<u32>>);
impl SignalPort for Tracked {
    fn aborted(&self) -> bool {
        self.0.aborted()
    }
    fn subscribe(&self, listener: AbortListener) -> ExtensionResult<Subscription> {
        Ok(Subscription::new(Rc::new(Removed(
            self.0.subscribe(listener)?,
            self.1.clone(),
        ))))
    }
}
/// The caller's real remover, not a replacement cancellation source.
struct Removed(Subscription, Rc<Cell<u32>>);
impl SubscriptionPort for Removed {
    fn unsubscribe(&self) -> ExtensionResult<()> {
        self.0.unsubscribe()?;
        self.1.set(self.1.get() + 1);
        Ok(())
    }
}

/// Creates a future that owns its chosen native operations and original signal.
pub(super) fn begin(
    io: Rc<dyn NativeIo>,
    context: &ExtensionCommandContext,
    millis: u64,
) -> ExtensionResult<Pending> {
    let mut removed = None;
    let signal = context.signal()?.map(|signal| {
        let count = Rc::new(Cell::new(0));
        removed = Some(count.clone());
        AbortSignal::new(Rc::new(Tracked(signal, count)))
    });
    Ok(Pending {
        work: Box::pin(async move { io.sleep(Duration::from_millis(millis), signal).await }),
        removed,
    })
}

/// Starts, finishes or drops a wait after the creating command has returned.
pub(super) async fn retained(
    io: Rc<dyn NativeIo>,
    slot: &RefCell<Option<Pending>>,
    context: &ExtensionCommandContext,
    action: &str,
) -> ExtensionResult<()> {
    if action == "start" {
        let mut pending = begin(io, context, 3_600_000)?;
        let ready = std::future::poll_fn(|cx| Poll::Ready(pending.work.as_mut().poll(cx))).await;
        if ready.is_ready() {
            return Err("wait completed before release".into());
        }
        *slot.borrow_mut() = Some(pending);
        context.get_system_prompt()?;
        return Ok(());
    }
    let mut pending = slot.borrow_mut().take().ok_or("no retained wait")?;
    if action == "finish" {
        return pending.finish().await;
    }
    if action != "drop" {
        return Err("unknown wait operation".into());
    }
    pending.work = Box::pin(std::future::ready(Ok(())));
    pending.check()
}
