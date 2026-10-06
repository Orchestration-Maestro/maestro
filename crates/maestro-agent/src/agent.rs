//! Atomic admission and independently owned runtime execution.
use crate::{
    AgentContext, AgentError, AgentListener, AgentMessage, AgentOptions, AgentState, Queue,
    QueueMode,
};
use maestro_models::{ApiStreamSimpleFunction, Cancellation, Model, SimpleStreamOptions};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
    task::{Context, Poll},
};
use tokio::sync::watch;

type Outcome = Result<Vec<AgentMessage>, AgentError>;
/// Identity of one ordered subscription.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SubscriptionId(u64);
/// Shared conversation owner with one directly admitted run.
#[derive(Clone)]
pub struct Agent {
    pub(crate) inner: Arc<Inner>,
}
pub(crate) struct Inner {
    pub stream_fn: ApiStreamSimpleFunction,
    pub model: Model,
    pub options: AgentOptions,
    // Only read or mutate state under this mutex; run callbacks, notifications,
    // wakes and cleanup of removed caller-owned values after releasing it.
    pub state: Mutex<Shared>,
}
pub(crate) struct Shared {
    pub context: AgentContext,
    pub running: bool,
    pub queues: crate::queues::Queues,
    pub streaming: Option<maestro_models::AssistantMessage>,
    pub listeners: Vec<(SubscriptionId, AgentListener)>,
    pub next_id: u64,
    pub completion: Option<watch::Receiver<Option<Outcome>>>,
    pub cancellation: Option<Cancellation>,
}
impl Inner {
    pub fn lock(&self) -> MutexGuard<'_, Shared> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }
}
/// Awaitable observation of owned execution; dropping it does not cancel work.
pub struct RunHandle {
    future: Pin<Box<dyn Future<Output = Outcome> + Send>>,
}
impl Future for RunHandle {
    type Output = Outcome;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_mut().future.as_mut().poll(cx)
    }
}
fn observe(mut receiver: watch::Receiver<Option<Outcome>>) -> RunHandle {
    RunHandle {
        future: Box::pin(async move {
            loop {
                if let Some(result) = receiver.borrow().clone() {
                    return result;
                }
                if receiver.changed().await.is_err() {
                    return Err(AgentError::RunFailed);
                }
            }
        }),
    }
}
impl Agent {
    /// Construct an idle shared owner without selecting a provider implicitly.
    pub fn new(stream_fn: ApiStreamSimpleFunction, model: Model, options: AgentOptions) -> Self {
        Self {
            inner: Arc::new(Inner {
                stream_fn,
                model,
                options: options.clone(),
                state: Mutex::new(Shared {
                    context: options.context,
                    running: false,
                    queues: {
                        let mut queues = crate::queues::Queues::default();
                        queues.steering_mode = options.steering_mode;
                        queues.follow_up_mode = options.follow_up_mode;
                        queues
                    },
                    streaming: None,
                    listeners: vec![],
                    next_id: 0,
                    completion: None,
                    cancellation: None,
                }),
            }),
        }
    }
    /// Obtain an owned snapshot with no mutable storage access.
    pub fn state(&self) -> AgentState {
        let state = self.inner.lock();
        AgentState {
            context: crate::run::snapshot_context(&state.context),
            is_running: state.running,
            streaming_message: state.streaming.as_ref().map(crate::run::snapshot_assistant),
        }
    }
    /// Admit input synchronously and start independent owned runtime work.
    /// Returns Busy or RuntimeUnavailable without changing history.
    /// Entry steering is polled after awaited initial turn and input events.
    pub fn prompt(
        &self,
        message: AgentMessage,
        options: SimpleStreamOptions,
    ) -> Result<RunHandle, AgentError> {
        self.start(Some(message), options)
    }
    /// Continue existing history without replaying its message events.
    /// Only a restart supplied by steering skips the entry steering poll.
    pub fn continue_run(&self, options: SimpleStreamOptions) -> Result<RunHandle, AgentError> {
        self.start(None, options)
    }
    fn start(
        &self,
        message: Option<AgentMessage>,
        mut options: SimpleStreamOptions,
    ) -> Result<RunHandle, AgentError> {
        let mut state = self.inner.lock();
        if state.running {
            return Err(AgentError::Busy);
        }
        let mut restart = false;
        if message.is_none() {
            match state.context.messages.last() {
                None => return Err(AgentError::NoUsableHistory),
                Some(AgentMessage::Model(maestro_models::Message::Assistant(_))) => {
                    if state.queues.get(Queue::Steering).is_empty()
                        && state.queues.get(Queue::FollowUp).is_empty()
                    {
                        return Err(AgentError::AssistantTail);
                    }
                    restart = true;
                }
                _ => {}
            }
        }
        let runtime =
            tokio::runtime::Handle::try_current().map_err(|_| AgentError::RuntimeUnavailable)?;
        let mut skip_initial_steering = false;
        let input = if restart {
            let mut input = state.queues.drain(Queue::Steering);
            skip_initial_steering = !input.is_empty();
            if input.is_empty() {
                input = state.queues.drain(Queue::FollowUp);
            }
            input
        } else {
            message.into_iter().collect()
        };
        let (sender, receiver) = watch::channel(None);
        state.running = true;
        let prior_cancellation = state.cancellation.replace(
            options
                .base
                .signal
                .get_or_insert_with(Cancellation::new)
                .clone(),
        );
        let prior_completion = state.completion.replace(receiver.clone());
        drop(state);
        drop(prior_cancellation);
        drop(prior_completion);
        let inner = self.inner.clone();
        let worker = runtime.spawn(crate::run::run(
            inner.clone(),
            input,
            skip_initial_steering,
            options,
        ));
        runtime.spawn(async move {
            let outcome = worker.await.unwrap_or(Err(AgentError::RunFailed));
            let (streaming, cancellation) = {
                let mut state = inner.lock();
                state.running = false;
                (state.streaming.take(), state.cancellation.take())
            };
            drop(streaming);
            drop(cancellation);
            sender.send_replace(Some(outcome));
        });
        Ok(observe(receiver))
    }
    /// Enqueue turn-boundary input without editing history or starting work.
    pub fn steer(&self, message: AgentMessage) {
        self.inner
            .lock()
            .queues
            .get(Queue::Steering)
            .push_back(message);
    }
    /// Enqueue later input without editing history or starting work.
    pub fn follow_up(&self, message: AgentMessage) {
        self.inner
            .lock()
            .queues
            .get(Queue::FollowUp)
            .push_back(message);
    }
    /// Read the independent policy of a selected queue.
    pub fn queue_mode(&self, queue: Queue) -> QueueMode {
        *self.inner.lock().queues.mode(queue)
    }
    /// Change subsequent polls without consuming or reordering records.
    pub fn set_queue_mode(&self, queue: Queue, mode: QueueMode) {
        *self.inner.lock().queues.mode(queue) = mode;
    }
    /// Inspect an owned FIFO snapshot without consuming it.
    pub fn queue(&self, queue: Queue) -> Vec<AgentMessage> {
        self.inner
            .lock()
            .queues
            .get(queue)
            .iter()
            .map(crate::run::snapshot_record)
            .collect()
    }
    /// Remove and return the selected FIFO in order, leaving the other alone.
    pub fn clear_queue(&self, queue: Queue) -> Vec<AgentMessage> {
        self.inner.lock().queues.get(queue).drain(..).collect()
    }
    /// Signal only active cooperative work, never clearing queues or aborting tasks.
    /// Idle abort has no effect; cancellation does not undo remote effects.
    pub fn abort(&self) {
        let cancellation = self.inner.lock().cancellation.clone();
        if let Some(signal) = cancellation {
            signal.cancel();
        }
    }
    /// Await the run observed when polled, including its owned task and sinks.
    /// This is not a fence against later submissions or application settlement.
    /// Abnormal prior execution returns RunFailed until a new run is admitted.
    pub async fn wait_for_idle(&self) -> Result<(), AgentError> {
        let completion = self.inner.lock().completion.clone();
        if let Some(receiver) = completion {
            observe(receiver).await.map(|_| ())
        } else {
            Ok(())
        }
    }
    /// Remove future event acceptance, leaving already snapshotted delivery awaited.
    /// Removing the same identity again has no effect. Listener cleanup runs unlocked.
    pub fn unsubscribe(&self, subscription: SubscriptionId) {
        let removed = {
            let mut state = self.inner.lock();
            state
                .listeners
                .iter()
                .position(|(id, _)| *id == subscription)
                .map(|index| state.listeners.remove(index))
        };
        drop(removed);
    }
    /// Register an ordered awaited sink without replay.
    pub fn subscribe(&self, listener: AgentListener) -> SubscriptionId {
        let mut state = self.inner.lock();
        let id = SubscriptionId(state.next_id);
        state.next_id += 1;
        state.listeners.push((id, listener));
        id
    }
}
