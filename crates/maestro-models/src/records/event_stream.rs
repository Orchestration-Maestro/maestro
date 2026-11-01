//! Competing-reader FIFO with an independently retained terminal result.

use std::collections::VecDeque;
use std::future::{Future, poll_fn};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Poll, Waker};

use super::types::{Execution, Local, Threaded};
mod execution;
pub use execution::StreamExecution;

/// Family-selected synchronous terminal classifier.
type Classifier<T, R, E> = <E as Execution>::Shared<<E as StreamExecution<T, R>>::Classifier>;
/// Family-selected synchronous final-result extractor.
type Extractor<T, R, E> = <E as Execution>::Shared<<E as StreamExecution<T, R>>::Extractor>;

/// Queued read with its assigned event and wake registration.
struct Reader<T> {
    /// Stable identity used to locate this read across polls.
    identity: Arc<()>,
    /// Whether this read still awaits assignment.
    pending: bool,
    /// Event assigned to this reader before its next poll.
    event: Option<T>,
    /// Latest wake registration for a pending read.
    waker: Option<Arc<Waker>>,
}

/// Wake registration for an independently observed final result.
struct Observer {
    /// Stable identity used to replace or remove this observation.
    identity: Arc<()>,
    /// Latest wake registration for this observation.
    waker: Arc<Waker>,
}

/// Shared FIFO, reader registrations and terminal-result state.
struct State<T, R> {
    /// Events awaiting an available reader.
    queue: VecDeque<T>,
    /// Competing readers in registration order.
    readers: VecDeque<Reader<T>>,
    /// Independent final-result observers.
    observers: Vec<Observer>,
    /// First published final result, shared without cloning under the lock.
    result: Option<Arc<R>>,
    /// Whether further event admission has stopped.
    ended: bool,
    /// Whether the terminal callback is still computing the result.
    extracting: bool,
}

/// Producer-owned FIFO whose readers compete for events, not final results.
pub struct EventStream<T, R = T, E: StreamExecution<T, R> = Threaded> {
    /// Shared mutable stream state.
    state: Arc<Mutex<State<T, R>>>,
    /// Caller callback recognizing a terminal event.
    is_complete: Classifier<T, R, E>,
    /// Caller callback extracting the final result.
    extract_result: Extractor<T, R, E>,
}

impl<T, R, E: StreamExecution<T, R>> Clone for EventStream<T, R, E> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            is_complete: self.is_complete.clone(),
            extract_result: self.extract_result.clone(),
        }
    }
}

/// Acquire stream state even after a prior lock holder panicked.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl<T: 'static, R: Clone + 'static> EventStream<T, R> {
    /// Construct a stream with caller-supplied terminal recognition and extraction.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn new(
        is_complete: impl Fn(&T) -> bool + Send + Sync + 'static,
        extract_result: impl Fn(&T) -> R + Send + Sync + 'static,
    ) -> Self
    where
        T: Send,
        R: Send + Sync,
    {
        Self::from_callbacks(
            Threaded::share(is_complete),
            Threaded::share(extract_result),
        )
    }

    /// Construct a stream with browser-local terminal callbacks.
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn new(
        is_complete: impl Fn(&T) -> bool + 'static,
        extract_result: impl Fn(&T) -> R + 'static,
    ) -> Self {
        Self::from_callbacks(
            Threaded::share(is_complete),
            Threaded::share(extract_result),
        )
    }

    /// Construct a native stream that may carry local items.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn new_local_items(
        is_complete: impl Fn(&T) -> bool + Send + Sync + 'static,
        extract_result: impl Fn(&T) -> R + Send + Sync + 'static,
    ) -> Self {
        Self::from_callbacks(
            Threaded::share(is_complete),
            Threaded::share(extract_result),
        )
    }

    /// Construct a browser stream that may carry local items.
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn new_local_items(
        is_complete: impl Fn(&T) -> bool + 'static,
        extract_result: impl Fn(&T) -> R + 'static,
    ) -> Self {
        Self::from_callbacks(
            Threaded::share(is_complete),
            Threaded::share(extract_result),
        )
    }
}

impl<T: 'static, R: Clone + 'static> EventStream<T, R, Local> {
    /// Construct a stream whose synchronous callbacks retain caller-local captures.
    #[must_use]
    pub fn new_local(
        is_complete: impl Fn(&T) -> bool + 'static,
        extract_result: impl Fn(&T) -> R + 'static,
    ) -> Self {
        Self::from_callbacks(
            std::rc::Rc::new(is_complete),
            std::rc::Rc::new(extract_result),
        )
    }
}

impl<T: 'static, R: Clone + 'static, E: StreamExecution<T, R>> EventStream<T, R, E> {
    /// Initialize stream state from family-selected callbacks.
    fn from_callbacks(
        is_complete: Classifier<T, R, E>,
        extract_result: Extractor<T, R, E>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                queue: VecDeque::new(),
                readers: VecDeque::new(),
                observers: Vec::new(),
                result: None,
                ended: false,
                extracting: false,
            })),
            is_complete,
            extract_result,
        }
    }

    /// Admit one event; the first terminal event also publishes its result.
    ///
    /// Late events are ignored without running classification or extraction.
    pub fn push(&self, event: T) {
        if lock(&self.state).ended {
            return;
        }
        let terminal = (self.is_complete)(&event);
        if terminal {
            let mut state = lock(&self.state);
            if state.ended {
                drop(state);
                return;
            }
            state.ended = true;
            state.extracting = true;
        }
        let mut result = terminal.then(|| Arc::new((self.extract_result)(&event)));
        let mut event = Some(event);
        let mut wakes = Vec::new();
        let mut observers = Vec::new();
        let mut state = lock(&self.state);
        if state.ended && !terminal {
            drop(state);
            return;
        }
        if terminal {
            state.extracting = false;
            if state.result.is_none() {
                state.result = result.take();
            }
            observers = std::mem::take(&mut state.observers);
        }
        if let Some(reader) = state.readers.iter_mut().find(|reader| reader.pending) {
            reader.pending = false;
            reader.event = event.take();
            wakes.extend(reader.waker.take());
        } else if let Some(event) = event.take() {
            state.queue.push_back(event);
        }
        if terminal {
            finish_readers(&mut state.readers, &mut wakes);
        }
        drop(state);
        wake(wakes, observers);
    }

    /// End iteration without discarding queued events or replacing a retained result.
    ///
    /// A missing result remains pending; a later explicit result can settle it.
    pub fn end(&self, result: Option<R>) {
        let mut result = result.map(Arc::new);
        let mut wakes = Vec::new();
        let observers;
        {
            let mut state = lock(&self.state);
            state.ended = true;
            if state.result.is_none() {
                state.result = result.take();
            }
            observers = if state.result.is_some() && !state.extracting {
                std::mem::take(&mut state.observers)
            } else {
                Vec::new()
            };
            if !state.extracting {
                finish_readers(&mut state.readers, &mut wakes);
            }
        }
        wake(wakes, observers);
    }

    /// Observe the next event, or EOF after the retained queue is drained.
    pub fn next(&self) -> impl Future<Output = Option<T>> + 'static {
        let mut reader = Reading {
            state: Arc::clone(&self.state),
            identity: Arc::new(()),
        };
        poll_fn(move |context| reader.poll(Arc::new(context.waker().clone())))
    }

    /// Observe the first final result independently of event consumption.
    pub fn result(&self) -> impl Future<Output = R> + 'static {
        let mut observer = Observing {
            state: Arc::clone(&self.state),
            identity: Arc::new(()),
        };
        poll_fn(move |context| observer.poll(Arc::new(context.waker().clone())))
    }
}

/// Complete pending reads with EOF and collect their wake registrations.
fn finish_readers<T>(readers: &mut VecDeque<Reader<T>>, wakes: &mut Vec<Arc<Waker>>) {
    for reader in readers.iter_mut().filter(|reader| reader.pending) {
        reader.pending = false;
        wakes.extend(reader.waker.take());
    }
}

/// Notify readers and result observers after releasing stream state.
fn wake(wakes: Vec<Arc<Waker>>, observers: Vec<Observer>) {
    for waker in wakes {
        waker.wake_by_ref();
    }
    for observer in observers {
        observer.waker.wake_by_ref();
    }
}

/// Future-owned registration for a competing event read.
struct Reading<T, R> {
    /// Stream state that supplies the next event.
    state: Arc<Mutex<State<T, R>>>,
    /// Stable registration identity retained across polls.
    identity: Arc<()>,
}

impl<T, R> Reading<T, R> {
    /// Assign a queued event, observe EOF or register a pending read.
    fn poll(&mut self, waker: Arc<Waker>) -> Poll<Option<T>> {
        let mut retired = None;
        let mut replaced = None;
        let outcome;
        let mut state = lock(&self.state);
        let position = state
            .readers
            .iter()
            .position(|reader| Arc::ptr_eq(&reader.identity, &self.identity));
        if let Some(position) = position {
            if state.readers[position].pending {
                replaced = state.readers[position].waker.replace(waker);
                outcome = Poll::Pending;
            } else {
                retired = state.readers.remove(position);
                outcome = Poll::Ready(retired.as_mut().and_then(|reader| reader.event.take()));
            }
        } else if let Some(event) = state.queue.pop_front() {
            outcome = Poll::Ready(Some(event));
        } else if state.ended && !state.extracting {
            outcome = Poll::Ready(None);
        } else {
            state.readers.push_back(Reader {
                identity: Arc::clone(&self.identity),
                pending: true,
                event: None,
                waker: Some(waker),
            });
            outcome = Poll::Pending;
        }
        drop(state);
        drop((retired, replaced));
        outcome
    }
}

/// Retire one future's registration before destroying caller-owned values.
macro_rules! remove_waiter {
    ($owner:ident, $collection:ident, $remove:ident) => {
        let retired = {
            let mut state = lock(&$owner.state);
            let position = state
                .$collection
                .iter()
                .position(|waiter| Arc::ptr_eq(&waiter.identity, &$owner.identity));
            position.$remove(|position| state.$collection.remove(position))
        };
        drop(retired);
    };
}

impl<T, R> Drop for Reading<T, R> {
    fn drop(&mut self) {
        remove_waiter!(self, readers, and_then);
    }
}

/// Future-owned registration for an independent final-result observation.
struct Observing<T, R> {
    /// Stream state that retains the terminal result.
    state: Arc<Mutex<State<T, R>>>,
    /// Stable observation identity retained across polls.
    identity: Arc<()>,
}

impl<T, R: Clone> Observing<T, R> {
    /// Observe a published result or register for later notification.
    fn poll(&mut self, waker: Arc<Waker>) -> Poll<R> {
        let mut replaced = None;
        let result;
        let mut state = lock(&self.state);
        result = if state.extracting {
            None
        } else {
            state.result.as_ref().map(Arc::clone)
        };
        if result.is_none() {
            if let Some(observer) = state
                .observers
                .iter_mut()
                .find(|observer| Arc::ptr_eq(&observer.identity, &self.identity))
            {
                replaced = Some(std::mem::replace(&mut observer.waker, waker));
            } else {
                state.observers.push(Observer {
                    identity: Arc::clone(&self.identity),
                    waker,
                });
            }
        }
        drop(state);
        drop(replaced);
        result.map_or(Poll::Pending, |result| Poll::Ready((*result).clone()))
    }
}

impl<T, R> Drop for Observing<T, R> {
    fn drop(&mut self) {
        remove_waiter!(self, observers, map);
    }
}

use super::types::{AssistantMessageEvent, SharedAssistantMessage};

/// Operations on an assistant stream whose original owner retains buffering and settlement.
pub trait AssistantMessageEventStreamPort {
    /// Admit an update or the first terminal event on the original stream.
    fn push(&self, event: AssistantMessageEvent);
    /// End the original stream without replacing its published result.
    fn end(&self, result: Option<SharedAssistantMessage>);
    /// Observe one competing-reader event or EOF.
    fn next(&self) -> super::types::BoxFuture<Option<AssistantMessageEvent>>;
    /// Observe the original independently settled final message.
    fn result(&self) -> super::types::BoxFuture<SharedAssistantMessage>;
}
impl AssistantMessageEventStreamPort
    for EventStream<AssistantMessageEvent, SharedAssistantMessage>
{
    fn push(&self, event: AssistantMessageEvent) {
        Self::push(self, event);
    }
    fn end(&self, result: Option<SharedAssistantMessage>) {
        Self::end(self, result);
    }
    fn next(&self) -> super::types::BoxFuture<Option<AssistantMessageEvent>> {
        Box::pin(Self::next(self))
    }
    fn result(&self) -> super::types::BoxFuture<SharedAssistantMessage> {
        Box::pin(Self::result(self))
    }
}
/// Assistant-event stream retaining its original buffer and independently settled message.
#[derive(Clone)]
pub struct AssistantMessageEventStream {
    /// Original native or imported stream operations, shared by every alias.
    #[cfg(not(target_arch = "wasm32"))]
    inner: Arc<dyn AssistantMessageEventStreamPort + Send + Sync>,
    /// Browser operations retain the same local ownership as native browser callbacks.
    #[cfg(target_arch = "wasm32")]
    inner: std::rc::Rc<dyn AssistantMessageEventStreamPort>,
}
impl Default for AssistantMessageEventStream {
    fn default() -> Self {
        Self::new()
    }
}
impl AssistantMessageEventStream {
    /// Create the ordinary native FIFO with its existing terminal projection.
    #[must_use]
    pub fn new() -> Self {
        Self::from_port(EventStream::new(
            AssistantMessageEvent::is_terminal,
            |event| Arc::clone(event.message()),
        ))
    }
    /// Front an owned stream without adding a queue, reducer or result settlement.
    #[cfg(not(target_arch = "wasm32"))]
    #[must_use]
    pub fn from_port(port: impl AssistantMessageEventStreamPort + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(port),
        }
    }
    /// Front a browser-local stream without adding a second buffer.
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn from_port(port: impl AssistantMessageEventStreamPort + 'static) -> Self {
        Self {
            inner: std::rc::Rc::new(port),
        }
    }
    /// Admit an assistant update or terminal event.
    pub fn push(&self, event: AssistantMessageEvent) {
        self.inner.push(event);
    }
    /// End iteration with an optional first final message.
    pub fn end(&self, result: Option<SharedAssistantMessage>) {
        self.inner.end(result);
    }
    /// Observe one competing-reader event or EOF.
    pub fn next(&self) -> impl Future<Output = Option<AssistantMessageEvent>> + 'static {
        self.inner.next()
    }
    /// Observe the retained shared final message independently of event consumption.
    pub fn result(&self) -> impl Future<Output = SharedAssistantMessage> + 'static {
        self.inner.result()
    }
}
/// Construct an empty assistant event stream for a controlled producer.
#[must_use]
pub fn create_assistant_message_event_stream() -> AssistantMessageEventStream {
    AssistantMessageEventStream::new()
}
