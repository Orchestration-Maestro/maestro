//! Credential updates stay on their invocation's stack or future, never in static storage.
use super::{Message, Registry, pump::State};
use std::{
    cell::RefCell,
    rc::Weak,
    task::{Context, Waker},
};
use tokio::sync::mpsc::UnboundedSender;

thread_local! {
    /// Weak references find only the original pump on its own thread.
    static PUMPS: RefCell<Vec<Weak<State>>> = const { RefCell::new(Vec::new()) };
}
/// Makes the original local pump discoverable by its Send request endpoint.
pub(super) fn register(pump: Weak<State>) {
    PUMPS.with(|pumps| {
        let mut pumps = pumps.borrow_mut();
        pumps.retain(|pump| pump.strong_count() > 0);
        pumps.push(pump);
    });
}
impl Registry {
    /// Credential backends are invoked only on their creating pump thread.
    pub(crate) fn on_pump<T>(
        requests: &UnboundedSender<Message>,
        use_registry: impl FnOnce(&Self) -> T,
    ) -> Result<T, String> {
        let pump = PUMPS.with(|pumps| {
            pumps
                .borrow()
                .iter()
                .filter_map(Weak::upgrade)
                .find(|pump| {
                    pump.registry
                        .requests
                        .upgrade()
                        .is_some_and(|owner| owner.same_channel(requests))
                })
        });
        pump.map(|pump| use_registry(&pump.registry))
            .ok_or_else(|| "extension credential backend requires its original pump thread".into())
    }
}
/// One async update request holds only owned data and its ordinary async reply.
pub(crate) struct AsyncRequest {
    /// Actual current text supplied by the author backend.
    pub(crate) current: Option<String>,
    /// Native replacement or exact native failure.
    pub(crate) reply: tokio::sync::oneshot::Sender<Result<Option<String>, String>>,
}
/// The scoped operand service retains messages, never the borrowed update future.
pub(super) struct AsyncFrame {
    /// At most one invocation of the native `FnOnce` update is permitted.
    pub(super) request: Option<AsyncRequest>,
    /// Wakes the native future that owns the still-borrowed update.
    pub(super) waker: Waker,
    /// Whether this single-use operand has been invoked.
    used: bool,
}
/// Identity and Send admission of one borrowed lock-update operand.
#[derive(Clone)]
pub(crate) struct Endpoint {
    /// Stable invocation identity, shared with the scoped operand service.
    pub(crate) id: u64,
    /// The original local receiver, not an alternative pump.
    pub(crate) requests: UnboundedSender<Message>,
}
impl Endpoint {
    /// Calls the update while it is still borrowed by its native synchronous caller.
    pub(crate) fn update(&self, current: Option<String>) -> Result<Option<String>, String> {
        let (reply, result) = std::sync::mpsc::channel();
        self.requests
            .send(Message::LockUpdate(self.id, current, reply))
            .map_err(|_| "extension lock update ended")?;
        result
            .recv()
            .map_err(|_| "extension lock update reply ended")?
    }
    /// Awaits the update whose borrowed native future owns this scope.
    pub(crate) async fn update_async(
        &self,
        current: Option<String>,
    ) -> Result<Option<String>, String> {
        let (reply, result) = tokio::sync::oneshot::channel();
        self.requests
            .send(Message::AsyncLockUpdate(
                self.id,
                AsyncRequest { current, reply },
            ))
            .map_err(|_| "extension lock update ended")?;
        result
            .await
            .map_err(|_| "extension lock update reply ended")?
    }
}
impl State {
    /// Routes owned requests to the future that still owns the borrowed update.
    pub(super) fn async_update(&self, id: u64, request: AsyncRequest) {
        let mut frames = self.synchronous.async_frames.borrow_mut();
        if let Some(frame) = frames.get_mut(&id)
            && !frame.used
        {
            frame.used = true;
            frame.request = Some(request);
            let waker = frame.waker.clone();
            drop(frames);
            waker.wake();
        } else {
            drop(frames);
            let _ = request
                .reply
                .send(Err("extension lock update ended".into()));
        }
    }
}
/// Keeps only the Send endpoint in the native borrowed future.
pub(crate) struct Scope {
    /// The identity is allocated on the first poll on the original pump.
    pub(crate) endpoint: Endpoint,
}
impl Scope {
    /// Creates one async invocation scope without retaining a borrowed callback.
    pub(crate) fn new(registry: &Registry, context: &Context<'_>) -> Result<Self, String> {
        let pump = registry
            .pump
            .upgrade()
            .ok_or("extension resource owner ended")?;
        let id = pump.synchronous.allocate()?;
        pump.synchronous.async_frames.borrow_mut().insert(
            id,
            AsyncFrame {
                request: None,
                waker: context.waker().clone(),
                used: false,
            },
        );
        Ok(Self {
            endpoint: Endpoint {
                id,
                requests: registry.requests()?,
            },
        })
    }
    /// Takes one request and refreshes the future's wakeup outside every author callback.
    pub(crate) fn request(
        &self,
        registry: &Registry,
        context: &Context<'_>,
    ) -> Option<AsyncRequest> {
        let pump = registry.pump.upgrade()?;
        let mut frames = pump.synchronous.async_frames.borrow_mut();
        let frame = frames.get_mut(&self.endpoint.id)?;
        frame.waker.clone_from(context.waker());
        frame.request.take()
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let _ = self
            .endpoint
            .requests
            .send(Message::EndLockUpdate(self.endpoint.id));
    }
}
