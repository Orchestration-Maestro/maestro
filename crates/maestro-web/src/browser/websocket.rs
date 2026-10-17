//! Browser complete-message socket adapter.
#[cfg(any(test, target_arch = "wasm32"))]
use std::{
    cell::RefCell,
    collections::VecDeque,
    future::poll_fn,
    io,
    rc::{Rc, Weak},
    task::{Poll, Waker},
};

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::browser_web_socket;

/// One native socket notification.
#[cfg(any(test, target_arch = "wasm32"))]
enum Event {
    /// Upgrade completed.
    Open,
    /// Complete data message.
    Data(Vec<u8>),
    /// Native transport failure.
    Error(io::Error),
    /// End of stream.
    Close,
}
/// Per-connection notifications retained across cancelled reads.
#[cfg(any(test, target_arch = "wasm32"))]
#[derive(Default)]
struct Queue {
    /// Ordered notifications.
    events: VecDeque<Event>,
    /// Current reader's wake destination.
    waiter: Option<Waker>,
}
/// Deliver without invoking caller-controlled wake/drop under a borrow.
#[cfg(any(test, target_arch = "wasm32"))]
fn push(weak: &Weak<RefCell<Queue>>, event: Event) {
    if let Some(queue) = weak.upgrade() {
        let wake = {
            let mut queue = queue.borrow_mut();
            queue.events.push_back(event);
            queue.waiter.take()
        };
        if let Some(wake) = wake {
            wake.wake();
        }
    }
}
/// Observe the next queued notification with cancellation-safe storage.
#[cfg(any(test, target_arch = "wasm32"))]
async fn next(queue: &Rc<RefCell<Queue>>) -> Event {
    poll_fn(|cx| {
        let waker = cx.waker().clone();
        let (event, previous) = {
            let mut queue = queue.borrow_mut();
            let event = queue.events.pop_front();
            let previous = queue.waiter.replace(waker);
            (event, previous)
        };
        drop(previous);
        event.map_or(Poll::Pending, Poll::Ready)
    })
    .await
}

#[cfg(test)]
mod tests;
