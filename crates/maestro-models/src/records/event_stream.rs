//! Competing-reader FIFO with an independently retained terminal result.

use std::collections::VecDeque;
use std::future::{Future, poll_fn};
use std::sync::{Arc, Mutex, MutexGuard};
use std::task::{Poll, Waker};

#[cfg(not(target_arch = "wasm32"))]
type Classifier<T> = Arc<dyn Fn(&T) -> bool + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type Classifier<T> = Arc<dyn Fn(&T) -> bool>;
#[cfg(not(target_arch = "wasm32"))]
type Extractor<T, R> = Arc<dyn Fn(&T) -> R + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type Extractor<T, R> = Arc<dyn Fn(&T) -> R>;

struct Reader<T> {
    identity: Arc<()>,
    pending: bool,
    event: Option<T>,
    waker: Option<Arc<Waker>>,
}

struct Observer {
    identity: Arc<()>,
    waker: Arc<Waker>,
}

struct State<T, R> {
    queue: VecDeque<T>,
    readers: VecDeque<Reader<T>>,
    observers: Vec<Observer>,
    result: Option<Arc<R>>,
    ended: bool,
    extracting: bool,
}

/// Producer-owned FIFO whose readers compete for events, not final results.
pub struct EventStream<T, R = T> {
    state: Arc<Mutex<State<T, R>>>,
    is_complete: Classifier<T>,
    extract_result: Extractor<T, R>,
}

impl<T, R> Clone for EventStream<T, R> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            is_complete: Arc::clone(&self.is_complete),
            extract_result: Arc::clone(&self.extract_result),
        }
    }
}

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
        Self::from_callbacks(Arc::new(is_complete), Arc::new(extract_result))
    }

    /// Construct a stream with browser-local terminal callbacks.
    #[cfg(target_arch = "wasm32")]
    #[must_use]
    pub fn new(
        is_complete: impl Fn(&T) -> bool + 'static,
        extract_result: impl Fn(&T) -> R + 'static,
    ) -> Self {
        Self::from_callbacks(Arc::new(is_complete), Arc::new(extract_result))
    }

    fn from_callbacks(is_complete: Classifier<T>, extract_result: Extractor<T, R>) -> Self {
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

fn finish_readers<T>(readers: &mut VecDeque<Reader<T>>, wakes: &mut Vec<Arc<Waker>>) {
    for reader in readers.iter_mut().filter(|reader| reader.pending) {
        reader.pending = false;
        wakes.extend(reader.waker.take());
    }
}

fn wake(wakes: Vec<Arc<Waker>>, observers: Vec<Observer>) {
    for waker in wakes {
        waker.wake_by_ref();
    }
    for observer in observers {
        observer.waker.wake_by_ref();
    }
}

struct Reading<T, R> {
    state: Arc<Mutex<State<T, R>>>,
    identity: Arc<()>,
}

impl<T, R> Reading<T, R> {
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

impl<T, R> Drop for Reading<T, R> {
    fn drop(&mut self) {
        let retired = {
            let mut state = lock(&self.state);
            let position = state
                .readers
                .iter()
                .position(|reader| Arc::ptr_eq(&reader.identity, &self.identity));
            position.and_then(|position| state.readers.remove(position))
        };
        drop(retired);
    }
}

struct Observing<T, R> {
    state: Arc<Mutex<State<T, R>>>,
    identity: Arc<()>,
}

impl<T, R: Clone> Observing<T, R> {
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
        let retired = {
            let mut state = lock(&self.state);
            let position = state
                .observers
                .iter()
                .position(|observer| Arc::ptr_eq(&observer.identity, &self.identity));
            position.map(|position| state.observers.remove(position))
        };
        drop(retired);
    }
}

use super::types::{AssistantMessageEvent, SharedAssistantMessage};

/// Assistant-event FIFO with an independently observable shared final message.
#[derive(Clone)]
pub struct AssistantMessageEventStream {
    inner: EventStream<AssistantMessageEvent, SharedAssistantMessage>,
}
impl Default for AssistantMessageEventStream {
    fn default() -> Self {
        Self::new()
    }
}
impl AssistantMessageEventStream {
    /// Create an empty assistant stream.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: EventStream::new(
                |event| {
                    matches!(
                        event,
                        AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
                    )
                },
                |event| match event {
                    AssistantMessageEvent::Done { message, .. } => Arc::clone(message),
                    AssistantMessageEvent::Error { error, .. } => Arc::clone(error),
                    AssistantMessageEvent::Start { partial }
                    | AssistantMessageEvent::TextStart { partial, .. }
                    | AssistantMessageEvent::TextDelta { partial, .. }
                    | AssistantMessageEvent::TextEnd { partial, .. }
                    | AssistantMessageEvent::ThinkingStart { partial, .. }
                    | AssistantMessageEvent::ThinkingDelta { partial, .. }
                    | AssistantMessageEvent::ThinkingEnd { partial, .. }
                    | AssistantMessageEvent::ToolcallStart { partial, .. }
                    | AssistantMessageEvent::ToolcallDelta { partial, .. }
                    | AssistantMessageEvent::ToolcallEnd { partial, .. } => Arc::clone(partial),
                },
            ),
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
