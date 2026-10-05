#![allow(dead_code)]

use super::block_on;
use maestro_models::*;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context as TaskContext, Poll, Wake, Waker},
};

pub fn model() -> Model {
    Model {
        identity: ModelIdentity {
            provider: "local".into(),
            model: "test".into(),
            operation: "chat".into(),
        },
        protocol: "script".into(),
    }
}
pub fn context() -> Context {
    Context {
        system_prompt: Some("synthetic secret".into()),
        messages: vec![UserMessage {
            content: "hello".into(),
            timestamp: 1,
        }],
    }
}
pub fn registry(provider: Arc<dyn Provider>) -> Models {
    let mut models = Models::new(Arc::new(|| 73));
    models.register(model(), provider).unwrap();
    models
}
pub fn steps(updates: Vec<ProviderUpdate>) -> Script {
    Script::Steps(updates.into_iter().map(ScriptStep::Update).collect())
}
pub fn fixture(updates: Vec<ProviderUpdate>) -> (Models, Arc<ScriptedProvider>) {
    let fake = Arc::new(ScriptedProvider::new(vec![steps(updates)]));
    (registry(fake.clone()), fake)
}
pub fn collect(mut stream: ModelStream, expected_model: &Model, timestamp: u64) -> Vec<ModelEvent> {
    let mut events = Vec::new();
    while let Some(event) = block_on(stream.next()) {
        events.push(event);
    }
    assert_eq!(block_on(stream.next()), None);
    assert_contract(&events, expected_model, timestamp);
    events
}
pub fn run(updates: Vec<ProviderUpdate>) -> Vec<ModelEvent> {
    let (models, _) = fixture(updates);
    collect(
        models.stream(model(), context(), StreamOptions::default()),
        &model(),
        73,
    )
}
pub fn snapshot(event: &ModelEvent) -> &AssistantMessage {
    match event {
        ModelEvent::Start { partial }
        | ModelEvent::TextStart { partial, .. }
        | ModelEvent::TextDelta { partial, .. }
        | ModelEvent::TextEnd { partial, .. }
        | ModelEvent::ThinkingStart { partial, .. }
        | ModelEvent::ThinkingDelta { partial, .. }
        | ModelEvent::ThinkingEnd { partial, .. }
        | ModelEvent::ToolCallStart { partial, .. }
        | ModelEvent::ToolCallDelta { partial, .. }
        | ModelEvent::ToolCallEnd { partial, .. } => partial,
        ModelEvent::Done { message, .. } => message,
        ModelEvent::Error { error, .. } => error,
    }
}
pub fn terminal(events: &[ModelEvent]) -> &AssistantMessage {
    snapshot(events.last().unwrap())
}
pub fn trace(events: &[ModelEvent]) -> Vec<(&'static str, Option<usize>)> {
    events
        .iter()
        .map(|event| match event {
            ModelEvent::Start { .. } => ("start", None),
            ModelEvent::TextStart { content_index, .. } => ("text_start", Some(*content_index)),
            ModelEvent::TextDelta { content_index, .. } => ("text_delta", Some(*content_index)),
            ModelEvent::TextEnd { content_index, .. } => ("text_end", Some(*content_index)),
            ModelEvent::ThinkingStart { content_index, .. } => {
                ("thinking_start", Some(*content_index))
            }
            ModelEvent::ThinkingDelta { content_index, .. } => {
                ("thinking_delta", Some(*content_index))
            }
            ModelEvent::ThinkingEnd { content_index, .. } => ("thinking_end", Some(*content_index)),
            ModelEvent::ToolCallStart { content_index, .. } => ("tool_start", Some(*content_index)),
            ModelEvent::ToolCallDelta { content_index, .. } => ("tool_delta", Some(*content_index)),
            ModelEvent::ToolCallEnd { content_index, .. } => ("tool_end", Some(*content_index)),
            ModelEvent::Done { .. } => ("done", None),
            ModelEvent::Error { .. } => ("error", None),
        })
        .collect()
}
pub fn assert_contract(events: &[ModelEvent], expected_model: &Model, timestamp: u64) {
    assert!(!events.is_empty());
    let mut started = false;
    let mut open = HashMap::new();
    for (position, event) in events.iter().enumerate() {
        let message = snapshot(event);
        assert_eq!(
            (
                &message.provider,
                &message.protocol,
                &message.model,
                message.timestamp
            ),
            (
                &expected_model.identity.provider,
                &expected_model.protocol,
                &expected_model.identity.model,
                timestamp
            )
        );
        let (kind, index) = trace(std::slice::from_ref(event))[0];
        if kind == "start" {
            assert!(!started);
            assert_eq!(position, 0);
            started = true;
        }
        if kind.ends_with("_start") {
            assert!(started);
            assert!(
                open.insert(index.unwrap(), kind.split('_').next().unwrap())
                    .is_none()
            );
        }
        if kind.ends_with("_delta") || kind.ends_with("_end") {
            assert_eq!(open.get(&index.unwrap()).copied(), kind.split('_').next());
            if kind.ends_with("_end") {
                open.remove(&index.unwrap());
            }
        }
        match event {
            ModelEvent::Done { reason, message } => {
                assert!(started);
                assert!(open.is_empty());
                assert_eq!(position, events.len() - 1);
                assert!(matches!(
                    reason,
                    StopReason::Stop | StopReason::Length | StopReason::ToolUse
                ));
                assert_eq!(message.stop_reason, Some(*reason));
                assert_eq!(message.failure, None);
            }
            ModelEvent::Error { reason, error } => {
                assert_eq!(position, events.len() - 1);
                assert!(matches!(reason, StopReason::Error | StopReason::Aborted));
                assert_eq!(error.stop_reason, Some(*reason));
                assert!(error.failure.is_some());
                assert_eq!(
                    *reason == StopReason::Aborted,
                    error.failure == Some(Failure::Cancelled)
                );
            }
            _ => {
                assert_eq!(message.stop_reason, None);
                assert_eq!(message.failure, None);
            }
        }
    }
    assert!(matches!(
        events.last(),
        Some(ModelEvent::Done { .. } | ModelEvent::Error { .. })
    ));
}
pub fn done() -> ProviderUpdate {
    ProviderUpdate::Done {
        reason: StopReason::Stop,
    }
}
pub fn text(index: usize, value: &str) -> Vec<ProviderUpdate> {
    vec![
        ProviderUpdate::TextStart {
            content_index: index,
        },
        ProviderUpdate::TextDelta {
            content_index: index,
            delta: value.into(),
        },
        ProviderUpdate::TextEnd {
            content_index: index,
        },
    ]
}
pub fn tool_start(index: usize) -> ProviderUpdate {
    ProviderUpdate::ToolCallStart {
        content_index: index,
        id: format!("call-{index}"),
        name: "lookup".into(),
        replay_metadata: Some("opaque".into()),
    }
}
pub fn usage() -> Usage {
    Usage {
        input: 11,
        output: 7,
        cache_read: 3,
        cache_write: 2,
        total_tokens: 23,
    }
}

#[derive(Default)]
pub struct WakeCounter(pub AtomicUsize);
impl Wake for WakeCounter {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
pub fn poll<F: Future + ?Sized>(
    future: Pin<&mut F>,
    counter: &Arc<WakeCounter>,
) -> Poll<F::Output> {
    let waker = Waker::from(counter.clone());
    future.poll(&mut TaskContext::from_waker(&waker))
}
#[derive(Default)]
struct GateState {
    ready: bool,
    waker: Option<Waker>,
}
#[derive(Clone, Default)]
pub struct Gate(Arc<Mutex<GateState>>, pub Arc<AtomicUsize>);
impl Gate {
    pub fn release(&self) {
        let waker = {
            let mut state = self.0.lock().unwrap();
            state.ready = true;
            state.waker.take()
        };
        if let Some(waker) = waker {
            waker.wake();
        }
    }
    pub fn wait(&self) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(GateWait(self.clone()))
    }
}
struct GateWait(Gate);
impl Future for GateWait {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<()> {
        let mut state = self.0.0.lock().unwrap();
        if state.ready {
            Poll::Ready(())
        } else {
            state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
impl Drop for GateWait {
    fn drop(&mut self) {
        self.0.1.fetch_add(1, Ordering::SeqCst);
    }
}

pub struct DirectProvider {
    pub calls: AtomicUsize,
    pub polls: Arc<AtomicUsize>,
    pub drops: Arc<AtomicUsize>,
    pub supports_chat: bool,
    pub setup_failure: Option<Failure>,
    pub updates: Mutex<Vec<ProviderUpdate>>,
    pub cancel_on_read: Option<Cancellation>,
}
impl DirectProvider {
    pub fn new(updates: Vec<ProviderUpdate>) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            polls: Arc::new(AtomicUsize::new(0)),
            drops: Arc::new(AtomicUsize::new(0)),
            supports_chat: true,
            setup_failure: None,
            updates: Mutex::new(updates),
            cancel_on_read: None,
        }
    }
}
impl Provider for DirectProvider {
    fn supports(&self, operation: &str) -> bool {
        self.supports_chat && operation == "chat"
    }
    fn stream(
        &self,
        _: Model,
        _: Context,
        _: StreamOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Some(failure) = self.setup_failure {
            return Err(failure);
        }
        Ok(Box::new(DirectStream {
            updates: self.updates.lock().unwrap().clone().into(),
            polls: self.polls.clone(),
            drops: self.drops.clone(),
            cancel_on_read: self.cancel_on_read.clone(),
        }))
    }
}
struct DirectStream {
    updates: std::collections::VecDeque<ProviderUpdate>,
    polls: Arc<AtomicUsize>,
    drops: Arc<AtomicUsize>,
    cancel_on_read: Option<Cancellation>,
}
impl ProviderStream for DirectStream {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async move {
            self.polls.fetch_add(1, Ordering::SeqCst);
            if let Some(cancellation) = self.cancel_on_read.take() {
                cancellation.cancel();
            }
            self.updates.pop_front()
        })
    }
}
impl Drop for DirectStream {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}
