//! Shared producer-owned FIFO events with independent result observation.
use super::{
    diagnostics::ThrownValue,
    types::{AssistantMessage, AssistantMessageEvent},
};
use std::{
    collections::VecDeque,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
    task::{Context, Poll, Waker},
};

#[cfg(not(target_arch = "wasm32"))]
type Callback<T, R> = Arc<dyn Fn(&T) -> Result<R, ThrownValue> + Send + Sync>;
#[cfg(target_arch = "wasm32")]
type Callback<T, R> = Arc<dyn Fn(&T) -> Result<R, ThrownValue>>;
#[cfg(not(target_arch = "wasm32"))]
type Observation<T> = Pin<Box<dyn Future<Output = T> + Send>>;
#[cfg(target_arch = "wasm32")]
type Observation<T> = Pin<Box<dyn Future<Output = T>>>;

#[cfg(not(target_arch = "wasm32"))]
type Driver = Box<dyn Fn(&mut Context<'_>) + Send>;
#[cfg(target_arch = "wasm32")]
type Driver = Box<dyn Fn(&mut Context<'_>)>;

struct Promise<T> {
    value: Option<Arc<T>>,
    wakers: Vec<Waker>,
}
impl<T: Clone> Promise<T> {
    fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            value: None,
            wakers: vec![],
        }))
    }
    fn publish(promise: &Arc<Mutex<Self>>, value: T) -> Vec<Waker> {
        let mut p = promise.lock().unwrap_or_else(|p| p.into_inner());
        if p.value.is_some() {
            return vec![];
        }
        p.value = Some(Arc::new(value));
        std::mem::take(&mut p.wakers)
    }
    fn settle(promise: &Arc<Mutex<Self>>, value: T) {
        notify(Self::publish(promise, value));
    }
}
fn notify(wakes: Vec<Waker>) {
    for waker in wakes {
        waker.wake();
    }
}
struct Read<T>(Arc<Mutex<Promise<T>>>, Driver);
impl<T: Clone> Future for Read<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        (self.1)(cx);
        let waker = cx.waker().clone();
        let value = {
            let mut p = self.0.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(v) = &p.value {
                Some(v.clone())
            } else {
                if !p.wakers.iter().any(|w| w.will_wake(&waker)) {
                    p.wakers.push(waker);
                }
                None
            }
        };
        if let Some(value) = value {
            return Poll::Ready((*value).clone());
        }
        Poll::Pending
    }
}
struct Queue<T> {
    queue: VecDeque<T>,
    waiting: VecDeque<Arc<Mutex<Promise<Option<T>>>>>,
    done: bool,
    terminal_admitted: bool,
    cursors: VecDeque<Arc<Mutex<Cursor<T>>>>,
}
/// Shared FIFO producer handle with an independently observed final result.
pub struct EventStream<T, R = T> {
    queue: Arc<Mutex<Queue<T>>>,
    result: Arc<Mutex<Promise<R>>>,
    is_complete: Callback<T, bool>,
    extract_result: Callback<T, R>,
}
impl<T, R> Clone for EventStream<T, R> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            result: self.result.clone(),
            is_complete: self.is_complete.clone(),
            extract_result: self.extract_result.clone(),
        }
    }
}

/// Independent cursor; already registered reads remain reserved when abandoned.
pub struct AsyncIterator<T> {
    queue: Arc<Mutex<Queue<T>>>,
    cursor: Arc<Mutex<Cursor<T>>>,
}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Clone + Send + Sync + 'static, R: Clone + Send + Sync + 'static> EventStream<T, R> {
    /// Install completion and result callbacks without starting an executor.
    pub fn new(is_complete: Callback<T, bool>, extract_result: Callback<T, R>) -> Self {
        Self {
            queue: Arc::new(Mutex::new(Queue {
                queue: VecDeque::new(),
                waiting: VecDeque::new(),
                done: false,
                terminal_admitted: false,
                cursors: VecDeque::new(),
            })),
            result: Promise::new(),
            is_complete,
            extract_result,
        }
    }
    /// Push supplied data; callback errors retain their ordered state transitions.
    pub fn push(&self, event: T) -> Result<(), ThrownValue> {
        if self
            .queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .terminal_admitted
        {
            return Ok(());
        }
        let result = if (self.is_complete)(&event)? {
            {
                let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
                queue.terminal_admitted = true;
                queue.done = true;
            }
            match (self.extract_result)(&event) {
                Ok(result) => Some(result),
                Err(error) => {
                    self.queue.lock().unwrap_or_else(|p| p.into_inner()).done = true;
                    return Err(error);
                }
            }
        } else {
            None
        };
        let mut event = Some(event);
        let waiter = {
            let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
            q.done |= result.is_some();
            if let Some(waiter) = q.waiting.pop_front() {
                Some(Promise::publish(&waiter, event.take()))
            } else {
                q.queue.push_back(event.take().unwrap());
                None
            }
        };
        let delivery = waiter;
        let settlement = result.map(|result| Promise::publish(&self.result, result));
        if let Some(delivery) = delivery {
            notify(delivery);
        }
        if let Some(settlement) = settlement {
            notify(settlement);
        }
        Ok(())
    }
    /// Close iteration; an absent result leaves result observation unresolved.
    pub fn end(&self, result: Option<R>) {
        let waiting = {
            let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
            q.done = true;
            q.terminal_admitted = true;
            std::mem::take(&mut q.waiting)
        };
        if let Some(value) = result {
            Promise::settle(&self.result, value);
        }
        for waiter in waiting {
            Promise::settle(&waiter, None);
        }
    }
    /// Create an independent cursor sharing the FIFO.
    pub fn iter(&self) -> AsyncIterator<T> {
        AsyncIterator {
            queue: self.queue.clone(),
            cursor: Arc::new(Mutex::new(Cursor {
                reads: VecDeque::new(),
                exhausted: false,
                scheduled: false,
            })),
        }
    }
    /// Observe the result without draining events or starting producer work.
    pub fn result(&self) -> Observation<R> {
        Box::pin(Read(self.result.clone(), driver(&self.queue)))
    }
}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Clone + Send + Sync + 'static> AsyncIterator<T> {
    /// Request an owned read eagerly when the preceding read has settled.
    /// Polling any read or result advances this stream's scheduled continuations.
    /// Dropping an observation does not cancel its reserved delivery.
    #[expect(
        clippy::should_implement_trait,
        reason = "the asynchronous cursor returns an observation future rather than a synchronous Iterator item"
    )]
    pub fn next(&mut self) -> Observation<Option<T>> {
        let promise = Promise::new();
        let delivery = Promise::new();
        let (eager, exhausted, schedule) = {
            let mut cursor = self.cursor.lock().unwrap_or_else(|p| p.into_inner());
            let eager = cursor.reads.is_empty();
            let exhausted = cursor.exhausted;
            cursor.reads.push_back(CursorRead {
                delivery: delivery.clone(),
                observation: promise.clone(),
                registered: eager,
            });
            let schedule = !cursor.scheduled;
            cursor.scheduled = true;
            (eager, exhausted, schedule)
        };
        if schedule {
            self.queue
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .cursors
                .push_back(self.cursor.clone());
        }
        if eager {
            if exhausted {
                Promise::settle(&delivery, None);
            } else {
                request(&self.queue, &delivery);
            }
        }
        Box::pin(Read(promise, driver(&self.queue)))
    }
}
#[cfg(target_arch = "wasm32")]
impl<T: Clone + 'static, R: Clone + 'static> EventStream<T, R> {
    /// Install completion and result callbacks without starting an executor.
    pub fn new(is_complete: Callback<T, bool>, extract_result: Callback<T, R>) -> Self {
        Self {
            queue: Arc::new(Mutex::new(Queue {
                queue: VecDeque::new(),
                waiting: VecDeque::new(),
                done: false,
                terminal_admitted: false,
                cursors: VecDeque::new(),
            })),
            result: Promise::new(),
            is_complete,
            extract_result,
        }
    }
    /// Push supplied data; callback errors retain their ordered state transitions.
    pub fn push(&self, event: T) -> Result<(), ThrownValue> {
        if self
            .queue
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .terminal_admitted
        {
            return Ok(());
        }
        let result = if (self.is_complete)(&event)? {
            {
                let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
                queue.terminal_admitted = true;
                queue.done = true;
            }
            match (self.extract_result)(&event) {
                Ok(result) => Some(result),
                Err(error) => {
                    self.queue.lock().unwrap_or_else(|p| p.into_inner()).done = true;
                    return Err(error);
                }
            }
        } else {
            None
        };
        let mut event = Some(event);
        let waiter = {
            let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
            q.done |= result.is_some();
            if let Some(waiter) = q.waiting.pop_front() {
                Some(Promise::publish(&waiter, event.take()))
            } else {
                q.queue.push_back(event.take().unwrap());
                None
            }
        };
        let delivery = waiter;
        let settlement = result.map(|result| Promise::publish(&self.result, result));
        if let Some(delivery) = delivery {
            notify(delivery);
        }
        if let Some(settlement) = settlement {
            notify(settlement);
        }
        Ok(())
    }
    /// Close iteration; an absent result leaves result observation unresolved.
    pub fn end(&self, result: Option<R>) {
        let waiting = {
            let mut q = self.queue.lock().unwrap_or_else(|p| p.into_inner());
            q.done = true;
            q.terminal_admitted = true;
            std::mem::take(&mut q.waiting)
        };
        if let Some(value) = result {
            Promise::settle(&self.result, value);
        }
        for waiter in waiting {
            Promise::settle(&waiter, None);
        }
    }
    /// Create an independent cursor sharing the FIFO.
    pub fn iter(&self) -> AsyncIterator<T> {
        AsyncIterator {
            queue: self.queue.clone(),
            cursor: Arc::new(Mutex::new(Cursor {
                reads: VecDeque::new(),
                exhausted: false,
                scheduled: false,
            })),
        }
    }
    /// Observe the result without draining events or starting producer work.
    pub fn result(&self) -> Observation<R> {
        Box::pin(Read(self.result.clone(), driver(&self.queue)))
    }
}
#[cfg(target_arch = "wasm32")]
impl<T: Clone + 'static> AsyncIterator<T> {
    /// Request an owned read eagerly when the preceding read has settled.
    /// Polling any read or result advances this stream's scheduled continuations.
    /// Dropping an observation does not cancel its reserved delivery.
    #[expect(
        clippy::should_implement_trait,
        reason = "the asynchronous cursor returns an observation future rather than a synchronous Iterator item"
    )]
    pub fn next(&mut self) -> Observation<Option<T>> {
        let promise = Promise::new();
        let delivery = Promise::new();
        let (eager, exhausted, schedule) = {
            let mut cursor = self.cursor.lock().unwrap_or_else(|p| p.into_inner());
            let eager = cursor.reads.is_empty();
            let exhausted = cursor.exhausted;
            cursor.reads.push_back(CursorRead {
                delivery: delivery.clone(),
                observation: promise.clone(),
                registered: eager,
            });
            let schedule = !cursor.scheduled;
            cursor.scheduled = true;
            (eager, exhausted, schedule)
        };
        if schedule {
            self.queue
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .cursors
                .push_back(self.cursor.clone());
        }
        if eager {
            if exhausted {
                Promise::settle(&delivery, None);
            } else {
                request(&self.queue, &delivery);
            }
        }
        Box::pin(Read(promise, driver(&self.queue)))
    }
}

struct CursorRead<T> {
    delivery: Arc<Mutex<Promise<Option<T>>>>,
    observation: Arc<Mutex<Promise<Option<T>>>>,
    registered: bool,
}
struct Cursor<T> {
    reads: VecDeque<CursorRead<T>>,
    exhausted: bool,
    scheduled: bool,
}
#[cfg(not(target_arch = "wasm32"))]
fn driver<T: Clone + Send + Sync + 'static>(queue: &Arc<Mutex<Queue<T>>>) -> Driver {
    let queue = Arc::downgrade(queue);
    Box::new(move |cx| {
        if let Some(queue) = queue.upgrade() {
            advance(&queue, cx);
        }
    })
}
#[cfg(target_arch = "wasm32")]
fn driver<T: Clone + 'static>(queue: &Arc<Mutex<Queue<T>>>) -> Driver {
    let queue = Arc::downgrade(queue);
    Box::new(move |cx| {
        if let Some(queue) = queue.upgrade() {
            advance(&queue, cx);
        }
    })
}
fn advance<T: Clone>(queue: &Arc<Mutex<Queue<T>>>, cx: &mut Context<'_>) {
    let mut cursors = {
        let mut queue = queue.lock().unwrap_or_else(|p| p.into_inner());
        std::mem::take(&mut queue.cursors)
    };
    loop {
        let mut pending = VecDeque::new();
        let mut progressed = false;
        for cursor in cursors {
            let (delivery, register, exhausted) = {
                let mut state = cursor.lock().unwrap_or_else(|p| p.into_inner());
                let exhausted = state.exhausted;
                let Some(read) = state.reads.front_mut() else {
                    state.scheduled = false;
                    continue;
                };
                let register = !read.registered;
                read.registered = true;
                (read.delivery.clone(), register, exhausted)
            };
            if register {
                if exhausted {
                    Promise::settle(&delivery, None);
                } else {
                    request(queue, &delivery);
                }
            }
            let value = {
                let mut delivery = delivery.lock().unwrap_or_else(|p| p.into_inner());
                if delivery.value.is_none()
                    && !delivery.wakers.iter().any(|w| w.will_wake(cx.waker()))
                {
                    delivery.wakers.push(cx.waker().clone());
                }
                delivery.value.clone()
            };
            if let Some(value) = value {
                let observation = {
                    let mut state = cursor.lock().unwrap_or_else(|p| p.into_inner());
                    state.exhausted |= value.is_none();
                    state.reads.pop_front().unwrap().observation
                };
                progressed = true;
                Promise::settle(&observation, (*value).clone());
            }
            let mut state = cursor.lock().unwrap_or_else(|p| p.into_inner());
            if state.reads.is_empty() {
                state.scheduled = false;
            } else {
                pending.push_back(cursor.clone());
            }
        }
        if !progressed {
            queue
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .cursors
                .extend(pending);
            break;
        }
        cursors = pending;
    }
}

fn request<T: Clone>(queue: &Arc<Mutex<Queue<T>>>, promise: &Arc<Mutex<Promise<Option<T>>>>) {
    let ready = {
        let mut q = queue.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(v) = q.queue.pop_front() {
            Some(Some(v))
        } else if q.done {
            Some(None)
        } else {
            q.waiting.push_back(promise.clone());
            None
        }
    };
    if let Some(value) = ready {
        Promise::settle(promise, value);
    }
}
/// Assistant events retain the producer's shared mutable message objects.
#[derive(Clone)]
pub struct AssistantMessageEventStream(
    EventStream<AssistantMessageEvent, Arc<RwLock<AssistantMessage>>>,
);
impl Default for AssistantMessageEventStream {
    fn default() -> Self {
        Self::new()
    }
}
impl AssistantMessageEventStream {
    /// Create an empty stream recognizing done and error as terminal events.
    pub fn new() -> Self {
        Self(EventStream::new(
            Arc::new(|e| {
                Ok(matches!(
                    e,
                    AssistantMessageEvent::Done { .. } | AssistantMessageEvent::Error { .. }
                ))
            }),
            Arc::new(extract),
        ))
    }
    /// Push an event without normalizing its data.
    pub fn push(&self, event: AssistantMessageEvent) -> Result<(), ThrownValue> {
        self.0.push(event)
    }
    /// Close iteration, optionally supplying a final result.
    pub fn end(&self, result: Option<Arc<RwLock<AssistantMessage>>>) {
        self.0.end(result);
    }
    /// Create an independent FIFO cursor.
    pub fn iter(&self) -> AsyncIterator<AssistantMessageEvent> {
        self.0.iter()
    }
    /// Observe the shared result independently of event consumption.
    pub fn result(&self) -> Observation<Arc<RwLock<AssistantMessage>>> {
        self.0.result()
    }
}
fn extract(event: &AssistantMessageEvent) -> Result<Arc<RwLock<AssistantMessage>>, ThrownValue> {
    match event {
        AssistantMessageEvent::Done { message, .. } => Ok(message.clone()),
        AssistantMessageEvent::Error { error, .. } => Ok(error.clone()),
        _ => Err(super::diagnostics::error(
            "Unexpected event type for final result".into(),
        )),
    }
}
/// Construct an empty assistant event stream.
pub fn create_assistant_message_event_stream() -> AssistantMessageEventStream {
    AssistantMessageEventStream::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nonterminal_result_extraction_reports_exact_text() {
        let a: AssistantMessage = serde_json::from_value(serde_json::json!({"role":"assistant","content":[],"api":"a","provider":"p","model":"m","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":0})).unwrap();
        let error = extract(&AssistantMessageEvent::Start {
            partial: Arc::new(RwLock::new(a)),
        })
        .unwrap_err();
        assert_eq!(
            super::super::diagnostics::format_thrown_value(&error).unwrap(),
            "Unexpected event type for final result"
        );
    }
}
