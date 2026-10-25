//! Wake-driven synchronous guest replies on the sole local request receiver.
use super::{Message, Registry, pump::State};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use tokio::sync::mpsc::UnboundedSender;

/// Typed return values for synchronous guest invocations currently awaiting completion.
#[derive(Default)]
pub(super) struct Returns {
    /// Invocation identities are independent of resource-table slot reuse.
    next: Cell<u64>,
    /// Nested waits may receive an outer invocation's reply first.
    completed: RefCell<BTreeMap<u64, Result<(), String>>>,
}

impl Returns {
    /// Retains the single completion sent by each Store invocation.
    pub(super) fn complete(&self, id: u64, result: Result<(), String>) {
        self.completed.borrow_mut().insert(id, result);
    }
}

/// One admitted Store invocation's owned completion and pump lifetime.
pub(crate) struct Completion {
    /// Stable identity of the native caller waiting for this reply.
    id: u64,
    /// Taken once, so success, failure and abandonment settle exactly one reply.
    requests: Option<UnboundedSender<Message>>,
}

impl Completion {
    /// Sends the guest's result without a second reply receiver.
    pub(crate) fn complete(mut self, result: Result<(), String>) {
        if let Some(requests) = self.requests.take() {
            let _ = requests.send(Message::Completed(self.id, result));
        }
    }
}

impl Drop for Completion {
    fn drop(&mut self) {
        if let Some(requests) = self.requests.take() {
            let _ = requests.send(Message::Completed(
                self.id,
                Err("extension guest reply ended".to_owned()),
            ));
        }
    }
}

/// A synchronous wait sleeps until the current receiver or future wakes its thread.
struct ThreadWake(std::thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

impl Registry {
    /// Services reentrant native requests while the active Accessor enters the guest.
    pub(crate) fn invoke_sync(
        &self,
        start: impl FnOnce(Completion) -> Result<(), String>,
    ) -> Result<(), String> {
        let pump = self
            .pump
            .upgrade()
            .ok_or_else(|| "extension resource owner ended".to_owned())?;
        let requests = self
            .requests
            .upgrade()
            .filter(|requests| !requests.is_closed())
            .ok_or_else(|| "extension resource owner ended".to_owned())?;
        let id = pump.synchronous.next.get();
        pump.synchronous.next.set(
            id.checked_add(1)
                .ok_or_else(|| "extension invocation identity exhausted".to_owned())?,
        );
        let started = start(Completion {
            id,
            requests: Some(requests),
        });
        let result = pump.wait(id);
        started.and(result)
    }
}

impl State {
    /// Borrows the receiver only while polling, never while dispatching native code.
    fn wait(&self, id: u64) -> Result<(), String> {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let previous = self.waker.replace(waker.clone());
        let result = self.wait_for(id, &waker);
        self.waker.replace(previous.clone());
        previous.wake();
        result
    }

    /// Drives the current synchronous waiter without losing the suspended pump task's wakeup.
    fn wait_for(&self, id: u64, waker: &Waker) -> Result<(), String> {
        let mut context = Context::from_waker(waker);
        loop {
            if let Some(result) = self.synchronous.completed.borrow_mut().remove(&id) {
                return result;
            }
            match self.poll_next(&mut context) {
                Poll::Ready(true) => {}
                Poll::Ready(false) => return Err("extension resource owner ended".to_owned()),
                Poll::Pending => std::thread::park(),
            }
        }
    }
}
