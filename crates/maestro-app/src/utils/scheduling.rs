//! Work that starts at once and finishes on a task facility.
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Waker},
};

/// A task's remaining work.
pub type LocalTask = Pin<Box<dyn Future<Output = ()>>>;

/// Polls `task` once now, so its synchronous part runs before this returns, and hands it
/// to `spawn` when it has more to do; the facility polls it again with its own waker.
pub fn start_now(mut task: LocalTask, spawn: impl FnOnce(LocalTask)) {
    if task
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
        .is_pending()
    {
        spawn(task);
    }
}

/// Forwards admissions onto the caller's local task set, without running work on senders.
/// Closing the sender ends forwarding; already admitted work retains its own lifetime.
pub(crate) fn forward<T: 'static>(
    local: &std::rc::Rc<tokio::task::LocalSet>,
    work: impl Fn(T) -> LocalTask + 'static,
) -> tokio::sync::mpsc::UnboundedSender<T> {
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    drop(local.spawn_local(async move {
        while let Some(message) = receiver.recv().await {
            drop(tokio::task::spawn_local(work(message)));
        }
    }));
    sender
}
