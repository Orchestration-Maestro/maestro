//! Wake-driven synchronous guest replies on the sole local request receiver.
use super::{Message, Registry, pump::State};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    future::Future,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
};
use tokio::sync::mpsc::UnboundedSender;

/// Typed return values for synchronous guest invocations currently awaiting completion.
#[derive(Default)]
pub(super) struct Returns {
    /// Invocation identities are independent of resource-table slot reuse.
    next: Cell<u64>,
    /// Requests to borrowed async updates; the callbacks themselves stay in native futures.
    pub(super) async_frames: RefCell<BTreeMap<u64, super::credential_scope::AsyncFrame>>,
    /// Nested waits may receive an outer invocation's reply first.
    completed: RefCell<BTreeMap<u64, Result<(), String>>>,
}

impl Returns {
    /// Allocates an invocation identity shared by sync and async borrowed operands.
    pub(super) fn allocate(&self) -> Result<u64, String> {
        let id = self.next.get();
        self.next.set(
            id.checked_add(1)
                .ok_or("extension invocation identity exhausted")?,
        );
        Ok(id)
    }
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
        value: Option<&mut serde_json::Value>,
        start: impl FnOnce(Completion, crate::schema_reads::Endpoint) -> Result<(), String>,
    ) -> Result<(), String> {
        self.invoke_scoped(value.map_or(Operand::None, Operand::Json), start)
    }

    /// Services the typed operand while its native caller still borrows it.
    pub(crate) fn invoke_scoped(
        &self,
        value: Operand<'_>,
        start: impl FnOnce(Completion, crate::schema_reads::Endpoint) -> Result<(), String>,
    ) -> Result<(), String> {
        let pump = self
            .pump
            .upgrade()
            .ok_or_else(|| "extension resource owner ended".to_owned())?;
        let requests = self.requests()?;
        let id = pump.synchronous.allocate()?;
        let endpoint = crate::schema_reads::Endpoint {
            id,
            requests: requests.clone(),
        };
        let started = start(
            Completion {
                id,
                requests: Some(requests),
            },
            endpoint,
        );
        let result = pump.wait(id, value);
        started.and(result)
    }
}

impl State {
    /// Borrows the receiver only while polling, never while dispatching native code.
    fn wait(&self, id: u64, value: Operand<'_>) -> Result<(), String> {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let previous = self.waker.replace(waker.clone());
        let result = self.wait_for(id, &waker, value);
        self.waker.replace(previous.clone());
        previous.wake();
        result
    }

    /// Drives the current synchronous waiter without losing the suspended pump task's wakeup.
    fn wait_for(&self, id: u64, waker: &Waker, mut value: Operand<'_>) -> Result<(), String> {
        let mut context = Context::from_waker(waker);
        loop {
            if let Some(result) = self.synchronous.completed.borrow_mut().remove(&id) {
                return result;
            }
            // A blocking waiter cannot yield to replenish Tokio's cooperative budget.
            let mut next = std::pin::pin!(tokio::task::coop::unconstrained(std::future::poll_fn(
                |context| {
                    self.poll_next_using(context, &mut |message| {
                        self.scoped(message, id, &mut value)
                    })
                }
            )));
            match next.as_mut().poll(&mut context) {
                Poll::Ready(true) => {}
                Poll::Ready(false) => return Err("extension resource owner ended".to_owned()),
                Poll::Pending => std::thread::park(),
            }
        }
    }
}

/// A borrow belongs to exactly one native invocation, never to a resource-table entry.
pub(crate) enum Operand<'a> {
    /// A callback without a borrowed operand.
    None,
    /// The validator's actual value.
    Json(&'a mut serde_json::Value),
    /// The credential owner's non-Send native update.
    Lock(&'a mut dyn FnMut(Option<&str>) -> maestro_extensions::LockUpdate),
}

impl State {
    /// Answer only the operand belonging to this stack's synchronous invocation.
    fn scoped(&self, message: Message, id: u64, value: &mut Operand<'_>) -> Option<Message> {
        match message {
            Message::Routed(message, route) => {
                self.routes.borrow_mut().push(route);
                let remaining = self.scoped(*message, id, value);
                if let Some(message) = remaining {
                    self.dispatch(message);
                }
                self.routes.borrow_mut().pop();
            }
            Message::Json(target, request) => {
                if target == id
                    && let Operand::Json(value) = value
                {
                    request.answer(value, &self.registry);
                } else {
                    request.reject();
                }
            }
            Message::LockUpdate(target, current, reply) => {
                let result = if target == id
                    && let Operand::Lock(update) = value
                {
                    update(current.as_deref()).map_err(|error| error.to_string())
                } else {
                    Err("extension lock update ended".into())
                };
                let _ = reply.send(result);
            }
            message => return Some(message),
        }
        None
    }
}
