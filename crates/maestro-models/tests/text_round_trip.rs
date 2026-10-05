mod support;

use maestro_models::*;
use std::sync::Arc;
use support::block_on;

fn model() -> Model {
    Model {
        identity: ModelIdentity {
            provider: "test:provider/custom".into(),
            model: "text/model:v1".into(),
            operation: "chat".into(),
        },
        protocol: "test:protocol/v1".into(),
    }
}

fn context() -> Context {
    Context {
        system_prompt: Some("Reply with a greeting".into()),
        messages: vec![UserMessage {
            content: "hello".into(),
            timestamp: 17,
        }],
    }
}

fn registry() -> Models {
    Models::new(Arc::new(|| 42))
}

fn updates() -> Vec<ProviderUpdate> {
    vec![
        ProviderUpdate::TextDelta {
            delta: "hel".into(),
        },
        ProviderUpdate::TextDelta { delta: "lo".into() },
        ProviderUpdate::Done {
            usage: Usage::default(),
        },
    ]
}

fn message(
    text: Option<&str>,
    usage: Usage,
    reason: Option<StopReason>,
    failure: Option<Failure>,
) -> AssistantMessage {
    AssistantMessage {
        provider: "test:provider/custom".into(),
        protocol: "test:protocol/v1".into(),
        model: "text/model:v1".into(),
        timestamp: 42,
        content: text
            .map(|text| vec![TextContent { text: text.into() }])
            .unwrap_or_default(),
        usage,
        stop_reason: reason,
        failure,
    }
}

async fn collect(models: &Models, model: Model, context: Context) -> Vec<ModelEvent> {
    let mut stream = models.stream(model, context);
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    assert_eq!(stream.next().await, None);
    events
}

#[test]
fn text_round_trip_has_exact_events_and_requested_identity() {
    block_on(async {
        let fake = Arc::new(ScriptedProvider::new(vec![updates()]));
        let mut models = registry();
        models.register(model(), fake.clone()).unwrap();
        let events = collect(&models, model(), context()).await;
        let partial = |text| message(text, Usage::default(), None, None);
        let terminal = message(
            Some("hello"),
            Usage::default(),
            Some(StopReason::Stop),
            None,
        );
        assert_eq!(
            events,
            vec![
                ModelEvent::Start {
                    partial: partial(None)
                },
                ModelEvent::TextStart {
                    content_index: 0,
                    partial: partial(Some(""))
                },
                ModelEvent::TextDelta {
                    content_index: 0,
                    delta: "hel".into(),
                    partial: partial(Some("hel"))
                },
                ModelEvent::TextDelta {
                    content_index: 0,
                    delta: "lo".into(),
                    partial: partial(Some("hello"))
                },
                ModelEvent::TextEnd {
                    content_index: 0,
                    content: "hello".into(),
                    partial: message(Some("hello"), Usage::default(), None, None)
                },
                ModelEvent::Done {
                    reason: StopReason::Stop,
                    message: terminal
                },
            ]
        );
        assert_eq!(fake.calls(), vec![(model(), context())]);
        assert_eq!(fake.pending(), 0);
    });
}

fn final_usage() -> Usage {
    Usage {
        input: 11,
        output: 7,
        cache_read: 3,
        cache_write: 2,
        total_tokens: 23,
    }
}

#[test]
fn retained_events_do_not_change_with_content_or_usage() {
    block_on(async {
        let mut chunks = updates();
        chunks[2] = ProviderUpdate::Done {
            usage: final_usage(),
        };
        let fake = Arc::new(ScriptedProvider::new(vec![chunks]));
        let mut models = registry();
        models.register(model(), fake).unwrap();
        let mut stream = models.stream(model(), context());
        let start = stream.next().await.unwrap();
        let text_start = stream.next().await.unwrap();
        let first_delta = stream.next().await.unwrap();
        assert!(matches!(
            stream.next().await,
            Some(ModelEvent::TextDelta { .. })
        ));
        assert_eq!(
            stream.next().await,
            Some(ModelEvent::TextEnd {
                content_index: 0,
                content: "hello".into(),
                partial: message(Some("hello"), final_usage(), None, None),
            })
        );
        assert_eq!(
            stream.next().await,
            Some(ModelEvent::Done {
                reason: StopReason::Stop,
                message: message(Some("hello"), final_usage(), Some(StopReason::Stop), None),
            })
        );
        assert_eq!(
            start,
            ModelEvent::Start {
                partial: message(None, Usage::default(), None, None)
            }
        );
        assert_eq!(
            text_start,
            ModelEvent::TextStart {
                content_index: 0,
                partial: message(Some(""), Usage::default(), None, None)
            }
        );
        assert_eq!(
            first_delta,
            ModelEvent::TextDelta {
                content_index: 0,
                delta: "hel".into(),
                partial: message(Some("hel"), Usage::default(), None, None)
            }
        );
        let mut cloned = first_delta.clone();
        if let ModelEvent::TextDelta { partial, .. } = &mut cloned {
            partial.content[0].text.push_str("caller mutation");
            partial.usage = final_usage();
        }
        assert_ne!(cloned, first_delta);
        assert_eq!(
            first_delta,
            ModelEvent::TextDelta {
                content_index: 0,
                delta: "hel".into(),
                partial: message(Some("hel"), Usage::default(), None, None)
            }
        );
        assert_eq!(stream.next().await, None);
    });
}

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};

struct InspectingProvider {
    calls: AtomicUsize,
    polls: Arc<AtomicUsize>,
}

impl InspectingProvider {
    fn new() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            polls: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl Provider for InspectingProvider {
    fn supports(&self, operation: &str) -> bool {
        operation == "chat"
    }
    fn stream(
        &self,
        actual_model: Model,
        actual_context: Context,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        assert_eq!(actual_model, model());
        assert_eq!(actual_context, context());
        self.calls.fetch_add(1, Ordering::SeqCst);
        let text = actual_context.messages[0].content.clone();
        Ok(Box::new(CountingSource {
            updates: vec![
                ProviderUpdate::TextDelta { delta: text },
                ProviderUpdate::Done {
                    usage: final_usage(),
                },
            ]
            .into(),
            polls: self.polls.clone(),
        }))
    }
}

struct CountingSource {
    updates: VecDeque<ProviderUpdate>,
    polls: Arc<AtomicUsize>,
}

impl ProviderStream for CountingSource {
    fn next(&mut self) -> Pin<Box<dyn Future<Output = Option<ProviderUpdate>> + Send + '_>> {
        Box::pin(async move {
            self.polls.fetch_add(1, Ordering::SeqCst);
            self.updates.pop_front()
        })
    }
}

fn terminal(events: &[ModelEvent]) -> AssistantMessage {
    match events.last().unwrap() {
        ModelEvent::Done { message, .. } => message.clone(),
        ModelEvent::Error { error, .. } => error.clone(),
        event => panic!("expected terminal event, got {event:?}"),
    }
}

#[test]
fn completion_consumes_the_stream_once() {
    block_on(async {
        let streamed_provider = Arc::new(InspectingProvider::new());
        let completed_provider = Arc::new(InspectingProvider::new());
        let clocks = Arc::new(AtomicUsize::new(0));
        let sampled = clocks.clone();
        let mut streamed = Models::new(Arc::new(move || {
            sampled.fetch_add(1, Ordering::SeqCst);
            42
        }));
        let mut completed = registry();
        streamed
            .register(model(), streamed_provider.clone())
            .unwrap();
        completed
            .register(model(), completed_provider.clone())
            .unwrap();
        let mut stream = streamed.stream(model(), context());
        assert_eq!(streamed_provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(streamed_provider.polls.load(Ordering::SeqCst), 0);
        let start = stream.next().await.unwrap();
        assert_eq!(
            start,
            ModelEvent::Start {
                partial: message(None, Usage::default(), None, None)
            }
        );
        assert_eq!(streamed_provider.polls.load(Ordering::SeqCst), 1);
        let text_start = stream.next().await.unwrap();
        assert!(matches!(text_start, ModelEvent::TextStart { .. }));
        assert_eq!(streamed_provider.polls.load(Ordering::SeqCst), 1);
        let mut events = vec![start, text_start];
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        assert_eq!(stream.next().await, None);
        let result = completed.complete(model(), context()).await;
        assert_eq!(result, terminal(&events));
        assert_eq!(
            result,
            message(Some("hello"), final_usage(), Some(StopReason::Stop), None)
        );
        assert_eq!(streamed_provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(completed_provider.calls.load(Ordering::SeqCst), 1);
        assert_eq!(streamed_provider.polls.load(Ordering::SeqCst), 2);
        assert_eq!(completed_provider.polls.load(Ordering::SeqCst), 2);
        assert_eq!(clocks.load(Ordering::SeqCst), 1);
    });
}

fn requested_failure(request: &Model, failure: Failure) -> AssistantMessage {
    AssistantMessage {
        provider: request.identity.provider.clone(),
        protocol: request.protocol.clone(),
        model: request.identity.model.clone(),
        timestamp: 42,
        content: Vec::new(),
        usage: Usage::default(),
        stop_reason: Some(StopReason::Error),
        failure: Some(failure),
    }
}

async fn assert_failure(models: &Models, request: Model, input: Context, failure: Failure) {
    let expected = requested_failure(&request, failure);
    assert_eq!(
        collect(models, request.clone(), input.clone()).await,
        vec![ModelEvent::Error {
            reason: StopReason::Error,
            error: expected.clone()
        }]
    );
    assert_eq!(models.complete(request, input).await, expected);
}

#[test]
fn dispatch_uses_provider_model_and_operation_as_data() {
    block_on(async {
        let mut models = registry();
        let first = model();
        let mut second = first.clone();
        second.identity.provider = "other:provider/path".into();
        let mut third = first.clone();
        third.identity.model = "another/model:id".into();
        let mut non_chat = first.clone();
        non_chat.identity.operation = "embedding".into();
        let identities = [first, second, third, non_chat.clone()];
        let adapters: Vec<_> = identities
            .iter()
            .map(|_| Arc::new(ScriptedProvider::new(vec![updates()])))
            .collect();
        for (identity, adapter) in identities.iter().zip(&adapters) {
            models.register(identity.clone(), adapter.clone()).unwrap();
        }
        for (identity, adapter) in identities[..3].iter().zip(&adapters) {
            let result = models.complete(identity.clone(), context()).await;
            assert_eq!(result.provider, identity.identity.provider);
            assert_eq!(result.protocol, identity.protocol);
            assert_eq!(result.model, identity.identity.model);
            assert_eq!(
                result.content,
                vec![TextContent {
                    text: "hello".into()
                }]
            );
            assert_eq!(result.stop_reason, Some(StopReason::Stop));
            assert_eq!(adapter.calls(), vec![(identity.clone(), context())]);
        }
        assert_failure(&models, non_chat, context(), Failure::UnsupportedOperation).await;
        for adapter in &adapters[..3] {
            assert_eq!(adapter.calls().len(), 1);
        }
        assert_eq!(adapters[3].calls(), Vec::new());
        assert_eq!(adapters[3].pending(), 1);
    });
}

#[test]
fn unknown_identities_return_secret_safe_errors() {
    block_on(async {
        let fake = Arc::new(ScriptedProvider::new(vec![updates()]));
        let clocks = Arc::new(AtomicUsize::new(0));
        let sampled = clocks.clone();
        let mut models = Models::new(Arc::new(move || {
            sampled.fetch_add(1, Ordering::SeqCst);
            42
        }));
        models.register(model(), fake.clone()).unwrap();
        let sentinel = "SECRET_SENTINEL_abc123";
        let mut input = context();
        input.system_prompt = Some(sentinel.into());
        input.messages[0].content = sentinel.into();
        let mut unknown_provider = model();
        unknown_provider.identity.provider = sentinel.into();
        let mut unknown_model = model();
        unknown_model.identity.model = sentinel.into();
        for (request, failure) in [
            (unknown_provider, Failure::UnknownProvider),
            (unknown_model, Failure::UnknownModel),
        ] {
            assert_failure(&models, request, input.clone(), failure).await;
            assert!(!failure.to_string().contains(sentinel));
            assert!(failure.to_string().contains("unknown"));
            assert!(failure.to_string().contains("register"));
        }
        assert_eq!(fake.calls(), Vec::new());
        assert_eq!(fake.pending(), 1);
        assert_eq!(clocks.load(Ordering::SeqCst), 4);
    });
}

struct UnsupportedProvider(AtomicUsize);
impl Provider for UnsupportedProvider {
    fn supports(&self, _: &str) -> bool {
        false
    }
    fn stream(&self, _: Model, _: Context) -> Result<Box<dyn ProviderStream>, Failure> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Failure::AdapterFailed)
    }
}

#[test]
fn unimplemented_operations_and_protocol_mismatch_never_dispatch() {
    block_on(async {
        let fake = Arc::new(ScriptedProvider::new(vec![updates()]));
        let unsupported = Arc::new(UnsupportedProvider(AtomicUsize::new(0)));
        let mut models = registry();
        let mut non_chat = model();
        non_chat.identity.operation = "native:other/op".into();
        let mut unsupported_model = model();
        unsupported_model.identity.model = "unsupported:model/id".into();
        models.register(model(), fake.clone()).unwrap();
        models.register(non_chat.clone(), fake.clone()).unwrap();
        models
            .register(unsupported_model.clone(), unsupported.clone())
            .unwrap();
        let mut wrong_protocol = model();
        wrong_protocol.protocol = "wrong:protocol/path".into();
        for request in [non_chat, unsupported_model, wrong_protocol] {
            assert_failure(&models, request, context(), Failure::UnsupportedOperation).await;
        }
        assert_eq!(unsupported.0.load(Ordering::SeqCst), 0);
        assert_eq!(fake.calls(), Vec::new());
        assert_eq!(fake.pending(), 1);
        let display = Failure::UnsupportedOperation.to_string();
        assert!(display.contains("operation"));
        assert!(display.contains("protocol"));
    });
}

#[test]
fn duplicate_registration_preserves_the_original_adapter() {
    block_on(async {
        let original = Arc::new(ScriptedProvider::new(vec![updates()]));
        let other = Arc::new(ScriptedProvider::new(vec![updates()]));
        let mut models = registry();
        models.register(model(), original.clone()).unwrap();
        let mut conflicting = model();
        conflicting.protocol = "conflicting:protocol".into();
        assert_eq!(
            models.register(conflicting, other.clone()),
            Err(Failure::DuplicateModel)
        );
        assert_eq!(
            models.complete(model(), context()).await.stop_reason,
            Some(StopReason::Stop)
        );
        assert_eq!(original.calls(), vec![(model(), context())]);
        assert_eq!(other.calls(), Vec::new());
        let mut independent = registry();
        assert_failure(&independent, model(), context(), Failure::UnknownProvider).await;
        independent.register(model(), other.clone()).unwrap();
        assert_eq!(
            independent.complete(model(), context()).await.stop_reason,
            Some(StopReason::Stop)
        );
        assert_eq!(other.calls(), vec![(model(), context())]);
        assert_eq!(original.calls().len(), 1);
    });
}

#[test]
fn scripted_calls_pending_and_exhaustion_are_observable() {
    block_on(async {
        let fake = Arc::new(ScriptedProvider::new(vec![
            updates(),
            vec![ProviderUpdate::Done {
                usage: final_usage(),
            }],
        ]));
        let mut models = registry();
        models.register(model(), fake.clone()).unwrap();
        assert_eq!(fake.pending(), 2);
        assert_eq!(fake.calls(), Vec::new());
        assert_eq!(
            models.complete(model(), context()).await,
            message(
                Some("hello"),
                Usage::default(),
                Some(StopReason::Stop),
                None
            )
        );
        assert_eq!(fake.pending(), 1);
        let mut retained_calls = fake.calls();
        let mut second_context = context();
        second_context.messages[0].content = "second invocation".into();
        assert_eq!(
            collect(&models, model(), second_context.clone()).await,
            vec![
                ModelEvent::Start {
                    partial: message(None, Usage::default(), None, None)
                },
                ModelEvent::Done {
                    reason: StopReason::Stop,
                    message: message(None, final_usage(), Some(StopReason::Stop), None)
                },
            ]
        );
        assert_eq!(fake.pending(), 0);
        assert_eq!(retained_calls, vec![(model(), context())]);
        retained_calls[0].1.messages[0].content = "caller mutation".into();
        assert_eq!(
            fake.calls(),
            vec![(model(), context()), (model(), second_context.clone())]
        );
        assert_eq!(
            collect(&models, model(), context()).await,
            vec![ModelEvent::Error {
                reason: StopReason::Error,
                error: requested_failure(&model(), Failure::ScriptExhausted),
            }]
        );
        assert_eq!(
            fake.calls(),
            vec![
                (model(), context()),
                (model(), second_context),
                (model(), context())
            ]
        );
        assert_eq!(fake.pending(), 0);
    });
}

async fn caller(models: &Models) -> (Vec<ModelEvent>, AssistantMessage) {
    let events = collect(models, model(), context()).await;
    let completed = models.complete(model(), context()).await;
    (events, completed)
}

#[test]
fn replacing_the_adapter_requires_no_caller_changes() {
    block_on(async {
        let scripted_updates = || {
            vec![
                ProviderUpdate::TextDelta {
                    delta: "hello".into(),
                },
                ProviderUpdate::Done {
                    usage: final_usage(),
                },
            ]
        };
        let fake = Arc::new(ScriptedProvider::new(vec![
            scripted_updates(),
            scripted_updates(),
        ]));
        let inspecting = Arc::new(InspectingProvider::new());
        let mut scripted_models = registry();
        let mut inspecting_models = registry();
        scripted_models.register(model(), fake.clone()).unwrap();
        inspecting_models
            .register(model(), inspecting.clone())
            .unwrap();
        let scripted = caller(&scripted_models).await;
        let computed = caller(&inspecting_models).await;
        assert_eq!(scripted, computed);
        let expected = message(Some("hello"), final_usage(), Some(StopReason::Stop), None);
        assert_eq!(scripted.1, expected);
        assert_eq!(terminal(&scripted.0), expected);
        assert_eq!(
            fake.calls(),
            vec![(model(), context()), (model(), context())]
        );
        assert_eq!(fake.pending(), 0);
        assert_eq!(inspecting.calls.load(Ordering::SeqCst), 2);
        assert_eq!(inspecting.polls.load(Ordering::SeqCst), 4);
    });
}

struct FailingProvider {
    setup_failure: bool,
    calls: AtomicUsize,
    polls: Arc<AtomicUsize>,
}

impl Provider for FailingProvider {
    fn supports(&self, operation: &str) -> bool {
        operation == "chat"
    }
    fn stream(&self, _: Model, _: Context) -> Result<Box<dyn ProviderStream>, Failure> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.setup_failure {
            return Err(Failure::AdapterFailed);
        }
        Ok(Box::new(CountingSource {
            updates: vec![
                ProviderUpdate::TextDelta {
                    delta: "partial".into(),
                },
                ProviderUpdate::Error {
                    failure: Failure::AdapterFailed,
                },
                ProviderUpdate::Done {
                    usage: final_usage(),
                },
            ]
            .into(),
            polls: self.polls.clone(),
        }))
    }
}

#[test]
fn provider_failures_remain_terminal_model_events() {
    block_on(async {
        for setup_failure in [true, false] {
            let provider = Arc::new(FailingProvider {
                setup_failure,
                calls: AtomicUsize::new(0),
                polls: Arc::new(AtomicUsize::new(0)),
            });
            let mut models = registry();
            models.register(model(), provider.clone()).unwrap();
            let (events, completed) = caller(&models).await;
            let expected = message(
                if setup_failure { None } else { Some("partial") },
                Usage::default(),
                Some(StopReason::Error),
                Some(Failure::AdapterFailed),
            );
            assert_eq!(terminal(&events), expected);
            assert_eq!(completed, expected);
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(
                        event,
                        ModelEvent::Error {
                            reason: StopReason::Error,
                            ..
                        }
                    ))
                    .count(),
                1
            );
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, ModelEvent::Done { .. }))
            );
            if setup_failure {
                assert_eq!(
                    events,
                    vec![ModelEvent::Error {
                        reason: StopReason::Error,
                        error: expected
                    }]
                );
                assert_eq!(provider.polls.load(Ordering::SeqCst), 0);
            } else {
                assert_eq!(events.len(), 4);
                assert!(matches!(events[0], ModelEvent::Start { .. }));
                assert!(matches!(events[1], ModelEvent::TextStart { .. }));
                assert!(matches!(events[2], ModelEvent::TextDelta { .. }));
                assert_eq!(provider.polls.load(Ordering::SeqCst), 4);
            }
            assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
        }
        let fake = Arc::new(ScriptedProvider::new(vec![vec![ProviderUpdate::Error {
            failure: Failure::AdapterFailed,
        }]]));
        let mut models = registry();
        models.register(model(), fake).unwrap();
        assert_eq!(
            collect(&models, model(), context()).await,
            vec![ModelEvent::Error {
                reason: StopReason::Error,
                error: requested_failure(&model(), Failure::AdapterFailed)
            }]
        );
    });
}

#[test]
fn source_eof_without_terminal_update_is_not_success() {
    block_on(async {
        let incomplete = || {
            vec![ProviderUpdate::TextDelta {
                delta: "partial".into(),
            }]
        };
        let fake = Arc::new(ScriptedProvider::new(vec![incomplete(), incomplete()]));
        let mut models = registry();
        models.register(model(), fake.clone()).unwrap();
        let (events, completed) = caller(&models).await;
        let expected = message(
            Some("partial"),
            Usage::default(),
            Some(StopReason::Error),
            Some(Failure::IncompleteStream),
        );
        assert_eq!(
            events,
            vec![
                ModelEvent::Start {
                    partial: message(None, Usage::default(), None, None)
                },
                ModelEvent::TextStart {
                    content_index: 0,
                    partial: message(Some(""), Usage::default(), None, None)
                },
                ModelEvent::TextDelta {
                    content_index: 0,
                    delta: "partial".into(),
                    partial: message(Some("partial"), Usage::default(), None, None)
                },
                ModelEvent::Error {
                    reason: StopReason::Error,
                    error: expected.clone()
                },
            ]
        );
        assert_eq!(completed, expected);
        assert_eq!(fake.calls().len(), 2);
        assert_eq!(fake.pending(), 0);
        let empty = Arc::new(ScriptedProvider::new(vec![Vec::new()]));
        let mut empty_models = registry();
        empty_models.register(model(), empty).unwrap();
        assert_eq!(
            collect(&empty_models, model(), context()).await,
            vec![ModelEvent::Error {
                reason: StopReason::Error,
                error: requested_failure(&model(), Failure::IncompleteStream),
            }]
        );
    });
}
