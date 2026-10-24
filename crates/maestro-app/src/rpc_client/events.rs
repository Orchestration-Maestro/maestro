//! Event callbacks, their registrations and the waits for the end of a run.
use super::{Inner, RpcClient, RpcClientError, RpcEventListener};
use maestro_models::{ImageContent, records::listeners::Listeners};
use serde_json::Value;
use std::{cell::RefCell, convert::Infallible, future::Future, rc::Rc, time::Duration};
use tokio::{sync::oneshot, time::Instant};

/// How long a wait for the end of a run lasts unless its caller chooses.
const WAIT_TIMEOUT: Duration = Duration::from_secs(60);

/// What the child's driver hands every registration.
pub(super) enum ClientInput<'a> {
    /// A record the child wrote that answers no pending request.
    Event(&'a Value),
    /// The child was stopped or has ended; nothing more arrives from it.
    Ended,
}

/// One registration; every subscription gets a fresh one, so a callback registered twice
/// is called twice.
pub(super) type ClientListener = Rc<dyn Fn(&ClientInput<'_>) -> Result<(), RpcClientError>>;

/// The kept events, or the failure that ended a wait.
type WaitOutcome = Result<Vec<Value>, RpcClientError>;

/// What a wait keeps and how its timeout reads.
#[derive(Clone, Copy)]
enum WaitKind {
    /// Waits for the end of the run.
    Idle,
    /// Keeps every event through the end of the run.
    Collect,
}

/// A wait registered from its creation; dropping it removes the registration.
struct EventWait {
    /// The registration that settles `outcome`.
    _registration: Registration,
    /// The kept events, or the failure that ended the wait.
    outcome: oneshot::Receiver<WaitOutcome>,
    /// When the wait gives up; never when the chosen timeout does not fit the clock.
    deadline: Option<Instant>,
    /// What the wait keeps.
    kind: WaitKind,
    /// The state holding the diagnostic text a timeout reports.
    inner: Rc<Inner>,
}

/// What a wait's registration keeps until the wait settles.
struct WaitState {
    /// What the wait keeps.
    kind: WaitKind,
    /// When the wait gives up; nothing that arrives from then on counts.
    deadline: Option<Instant>,
    /// Settles the wait; taken when it does.
    settle: RefCell<Option<oneshot::Sender<WaitOutcome>>>,
    /// The events kept so far.
    events: RefCell<Vec<Value>>,
}

impl WaitState {
    /// Keeps `input` when the wait collects, and settles the wait when `input` ends it.
    fn observe(&self, input: &ClientInput<'_>) {
        let expired = self
            .deadline
            .is_some_and(|deadline| Instant::now() >= deadline);
        if expired || self.settle.borrow().is_none() {
            return;
        }
        let outcome = match input {
            ClientInput::Ended => Err(RpcClientError::Stopped),
            ClientInput::Event(event) => {
                if matches!(self.kind, WaitKind::Collect) {
                    self.events.borrow_mut().push((*event).clone());
                }
                if event["type"] != "agent_end" {
                    return;
                }
                Ok(self.events.take())
            }
        };
        if let Some(settle) = self.settle.take() {
            let _ = settle.send(outcome);
        }
    }
}

/// Removes one registration when dropped.
struct Registration(Box<dyn Fn()>);

impl Drop for Registration {
    fn drop(&mut self) {
        (self.0)();
    }
}

impl RpcClient {
    /// Registers `listener` for every later record the child writes that answers no
    /// pending request, in registration order. The returned operation removes this
    /// registration only; calling it again does nothing, and dropping it keeps the
    /// registration.
    pub fn on_event(&self, listener: RpcEventListener) -> impl Fn() + use<> {
        let registration: ClientListener = Rc::new(move |input: &ClientInput<'_>| match input {
            ClientInput::Event(event) => listener(event),
            ClientInput::Ended => Ok(()),
        });
        Listeners::subscribe(&self.inner.listeners, registration)
    }

    /// Waits for the next `agent_end` event, registering at once, so an event that arrived
    /// before the call does not count; `None` waits 60 seconds.
    ///
    /// # Errors
    /// Returns [`RpcClientError::IdleTimeout`] at the deadline, or
    /// [`RpcClientError::Stopped`] when the child is stopped or ends first.
    pub fn wait_for_idle(
        &self,
        timeout: Option<Duration>,
    ) -> impl Future<Output = Result<(), RpcClientError>> + use<> {
        let wait = self.prepare_wait(timeout, WaitKind::Idle);
        async move { finish_wait(wait).await.map(drop) }
    }

    /// Returns every event from the call through the next `agent_end` event, registering at
    /// once; `None` waits 60 seconds.
    ///
    /// # Errors
    /// Returns [`RpcClientError::CollectionTimeout`] at the deadline, or
    /// [`RpcClientError::Stopped`] when the child is stopped or ends first.
    pub fn collect_events(
        &self,
        timeout: Option<Duration>,
    ) -> impl Future<Output = Result<Vec<Value>, RpcClientError>> + use<> {
        finish_wait(self.prepare_wait(timeout, WaitKind::Collect))
    }

    /// Starts collecting events, sends the prompt, then returns the events through the end
    /// of its run; a refused prompt ends the collection.
    ///
    /// # Errors
    /// Returns the prompt's failure, or the failures of [`RpcClient::collect_events`].
    pub async fn prompt_and_wait(
        &self,
        message: &str,
        images: Option<Vec<ImageContent>>,
        timeout: Option<Duration>,
    ) -> Result<Vec<Value>, RpcClientError> {
        let events = self.collect_events(timeout);
        self.prompt(message, images).await?;
        events.await
    }

    /// Registers a wait of `kind` whose deadline starts now.
    fn prepare_wait(&self, timeout: Option<Duration>, kind: WaitKind) -> EventWait {
        let deadline = Instant::now().checked_add(timeout.unwrap_or(WAIT_TIMEOUT));
        let (settle, outcome) = oneshot::channel();
        let state = WaitState {
            kind,
            deadline,
            settle: RefCell::new(Some(settle)),
            events: RefCell::new(Vec::new()),
        };
        let listener: ClientListener = Rc::new(move |input: &ClientInput<'_>| {
            state.observe(input);
            Ok(())
        });
        let remove = Listeners::subscribe(&self.inner.listeners, listener);
        EventWait {
            _registration: Registration(Box::new(remove)),
            outcome,
            deadline,
            kind,
            inner: Rc::clone(&self.inner),
        }
    }
}

/// Waits for `wait`'s outcome or its deadline, then removes its registration.
async fn finish_wait(wait: EventWait) -> Result<Vec<Value>, RpcClientError> {
    let settled = match wait.deadline {
        Some(deadline) => tokio::time::timeout_at(deadline, wait.outcome).await.ok(),
        None => Some(wait.outcome.await),
    };
    match settled {
        Some(Ok(outcome)) => outcome,
        Some(Err(_)) => Err(RpcClientError::Stopped),
        None => {
            let stderr = wait.inner.state.borrow().stderr.clone();
            Err(match wait.kind {
                WaitKind::Idle => RpcClientError::IdleTimeout { stderr },
                WaitKind::Collect => RpcClientError::CollectionTimeout { stderr },
            })
        }
    }
}

impl Inner {
    /// Calls the registrations with `event` in order, including ones added meanwhile; the
    /// first failure ends this delivery.
    pub(super) fn dispatch(&self, event: &Value) {
        let _ = Listeners::try_for_each_live(&self.listeners, |listener| {
            listener(&ClientInput::Event(event))
        });
    }

    /// Ends every pending request and wait of the current child with
    /// [`RpcClientError::Stopped`].
    pub(super) fn fail_owned(&self) {
        let pending = std::mem::take(&mut self.state.borrow_mut().pending);
        drop(pending);
        let _ = Listeners::try_for_each_live(&self.listeners, |listener| {
            let _ = listener(&ClientInput::Ended);
            Ok::<(), Infallible>(())
        });
    }
}
