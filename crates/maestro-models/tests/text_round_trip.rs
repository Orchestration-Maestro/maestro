mod support;

use maestro_models::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use support::block_on;

fn model() -> Model {
    Model {
        identity: ModelIdentity {
            provider: "test:provider/custom".into(),
            model: "text/model:v1".into(),
            operation: "chat".into(),
        },
        protocol: "test:protocol/v1".into(),
        capabilities: RequestCapabilities::default(),
        headers: Default::default(),
        input: vec!["text".into()],
    }
}

fn context() -> Context {
    Context {
        system_prompt: Some("Reply with a greeting".into()),
        messages: vec![Message::User(UserMessage {
            content: vec![InputContent::Text(TextContent {
                text: "hello".into(),
                replay_metadata: None,
            })],
            timestamp: 17,
        })],
        tools: vec![],
    }
}

fn registry() -> Models {
    Models::new(Arc::new(|| 42))
}

fn updates() -> Vec<ProviderUpdate> {
    vec![
        ProviderUpdate::TextStart { content_index: 0 },
        ProviderUpdate::TextDelta {
            content_index: 0,
            delta: "hel".into(),
        },
        ProviderUpdate::TextDelta {
            content_index: 0,
            delta: "lo".into(),
        },
        ProviderUpdate::TextEnd {
            content_index: 0,
            replay_metadata: None,
        },
        ProviderUpdate::Done {
            reason: StopReason::Stop,
        },
    ]
}

async fn collect(models: &Models, model: Model, context: Context) -> Vec<ModelEvent> {
    let mut stream = models.stream(model.clone(), context, support::auth::local());
    let mut events = Vec::new();
    while let Some(event) = stream.next().await {
        events.push(event);
    }
    assert_eq!(stream.next().await, None);
    support::conformance::assert_contract(&events, &model, 42);
    events
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
        response_model: None,
        response_id: None,
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
    assert_eq!(
        models
            .complete(request, input, support::auth::local())
            .await,
        expected
    );
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
            .map(|_| {
                Arc::new(ScriptedProvider::new(vec![Script::Steps(
                    updates().into_iter().map(ScriptStep::Update).collect(),
                )]))
            })
            .collect();
        for (identity, adapter) in identities.iter().zip(&adapters) {
            models.register(identity.clone(), adapter.clone()).unwrap();
        }
        for (identity, adapter) in identities[..3].iter().zip(&adapters) {
            let result = models
                .complete(identity.clone(), context(), support::auth::local())
                .await;
            assert_eq!(result.provider, identity.identity.provider);
            assert_eq!(result.protocol, identity.protocol);
            assert_eq!(result.model, identity.identity.model);
            assert_eq!(
                result.content,
                vec![AssistantContent::Text(TextContent {
                    text: "hello".into(),
                    replay_metadata: None
                })]
            );
            assert_eq!(result.stop_reason, Some(StopReason::Stop));
            assert_eq!(adapter.calls()[0].model, *identity);
            assert_eq!(adapter.calls()[0].context, context());
        }
        assert_failure(&models, non_chat, context(), Failure::UnsupportedOperation).await;
        for adapter in &adapters[..3] {
            assert_eq!(adapter.calls().len(), 1);
        }
        assert!(adapters[3].calls().is_empty());
        assert_eq!(adapters[3].pending(), 1);
    });
}

#[test]
fn unknown_identities_return_secret_safe_errors() {
    block_on(async {
        let fake = Arc::new(ScriptedProvider::new(vec![Script::Steps(
            updates().into_iter().map(ScriptStep::Update).collect(),
        )]));
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
        if let Message::User(user) = &mut input.messages[0] {
            user.content = vec![InputContent::Text(TextContent {
                text: sentinel.into(),
                replay_metadata: None,
            })];
        }
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
        assert!(fake.calls().is_empty());
        assert_eq!(fake.pending(), 1);
        assert_eq!(clocks.load(Ordering::SeqCst), 4);
    });
}

struct UnsupportedProvider(AtomicUsize);
impl Provider for UnsupportedProvider {
    fn supports(&self, _: &str) -> bool {
        false
    }
    fn stream(
        &self,
        _: Model,
        _: Context,
        _: ProviderOptions,
    ) -> Result<Box<dyn ProviderStream>, Failure> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Failure::AdapterFailed)
    }
}

#[test]
fn unimplemented_operations_and_protocol_mismatch_never_dispatch() {
    block_on(async {
        let fake = Arc::new(ScriptedProvider::new(vec![Script::Steps(
            updates().into_iter().map(ScriptStep::Update).collect(),
        )]));
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
        assert!(fake.calls().is_empty());
        assert_eq!(fake.pending(), 1);
        let display = Failure::UnsupportedOperation.to_string();
        assert!(display.contains("operation"));
        assert!(display.contains("protocol"));
    });
}

#[test]
fn duplicate_registration_preserves_the_original_adapter() {
    block_on(async {
        let original = Arc::new(ScriptedProvider::new(vec![Script::Steps(
            updates().into_iter().map(ScriptStep::Update).collect(),
        )]));
        let other = Arc::new(ScriptedProvider::new(vec![Script::Steps(
            updates().into_iter().map(ScriptStep::Update).collect(),
        )]));
        let mut models = registry();
        models.register(model(), original.clone()).unwrap();
        let mut conflicting = model();
        conflicting.protocol = "conflicting:protocol".into();
        assert_eq!(
            models.register(conflicting, other.clone()),
            Err(Failure::DuplicateModel)
        );
        assert_eq!(
            models
                .complete(model(), context(), support::auth::local())
                .await
                .stop_reason,
            Some(StopReason::Stop)
        );
        assert_eq!(original.calls()[0].model, model());
        assert_eq!(original.calls()[0].context, context());
        assert!(other.calls().is_empty());
        let mut independent = registry();
        assert_failure(&independent, model(), context(), Failure::UnknownProvider).await;
        independent.register(model(), other.clone()).unwrap();
        assert_eq!(
            independent
                .complete(model(), context(), support::auth::local())
                .await
                .stop_reason,
            Some(StopReason::Stop)
        );
        assert_eq!(other.calls()[0].model, model());
        assert_eq!(other.calls()[0].context, context());
        assert_eq!(original.calls().len(), 1);
    });
}
