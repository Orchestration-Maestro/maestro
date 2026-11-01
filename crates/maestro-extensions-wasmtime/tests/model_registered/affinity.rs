//! Registered providers retain native execution families across their original pump boundary.
use super::*;

/// Thread-safe callers reach the same guest provider from outside its original thread.
pub(super) fn run() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    tokio::task::LocalSet::new().block_on(&runtime, async {
        let bus = bus();
        let (_owner, runner, _) =
            super::super::retained_components::loaded_on(bus.for_extension()).await;
        register(&runner, "create").await;
        local_refusal().await;
        for simple in [false, true] {
            let (entered, incoming) = mpsc::unbounded_channel();
            let common = options(entered);
            let stream = tokio::task::spawn_blocking(move || start(simple, common))
                .await
                .unwrap()
                .unwrap();
            pending(&stream);
            let observed = stream.clone();
            let consumer = tokio::task::spawn_blocking(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .build()
                    .unwrap();
                runtime.block_on(check(&observed, "keep"));
            });
            finish_ended_producer(&runner, incoming).await;
            consumer.await.unwrap();
        }
        unregister_api_providers("provider:compiled-stream");
        bus.clear();
    });
}

/// An off-origin local caller receives the explicit affinity error instead of moving captures.
async fn local_refusal() {
    let provider = get_api_provider("guest-owned-stream").unwrap();
    let error = tokio::task::spawn_blocking(move || {
        let (model, context) = operands(false);
        provider
            .stream
            .call(
                model,
                context,
                ProviderInvocation::Local(
                    None,
                    Rc::new(|_| panic!("no off-thread local admission")),
                ),
            )
            .err()
            .unwrap()
            .message
    })
    .await
    .unwrap();
    assert_eq!(
        error,
        "local guest-provider invocation requires the extension's original pump thread"
    );
}

/// Original threaded hooks still await their caller-owned readiness gates.
fn options(entered: mpsc::UnboundedSender<(&'static str, oneshot::Sender<()>)>) -> StreamOptions {
    let payload = entered.clone();
    StreamOptions {
        temperature: Some(-0.0),
        timeout_ms: Some(17.5),
        max_retries: Some(2.5),
        signal: Some(Cancellation::new()),
        on_payload: Some(Arc::new(move |input, model| {
            assert_eq!(model.id, "registered-case");
            assert_eq!(
                input.payload,
                json!({"original":true,"unchanged":"neighbor"})
            );
            let entered = payload.clone();
            Box::pin(async move {
                gate(&entered, "payload").await;
                Ok(input)
            })
        })),
        on_response: Some(Arc::new(move |response, model| {
            assert_eq!(model.id, "registered-case");
            assert_eq!(response.status.to_bits(), 207.5_f64.to_bits());
            assert_eq!(response.headers["x-native"], "literal");
            let entered = entered.clone();
            Box::pin(async move {
                gate(&entered, "response").await;
                Ok(())
            })
        })),
        ..Default::default()
    }
}

/// Invoke from the foreign thread without capturing any caller-local operand.
fn start(
    simple: bool,
    common: StreamOptions,
) -> Result<AssistantMessageEventStream, DiagnosticErrorInfo> {
    let provider = get_api_provider("guest-owned-stream").unwrap();
    let (model, context) = operands(simple);
    if simple {
        provider.stream_simple.call(
            model,
            context,
            ProviderInvocation::Threaded(Some(simple_options(common)), None),
        )
    } else {
        provider.stream.call(
            model,
            context,
            ProviderInvocation::Threaded(Some(raw_options(common)), None),
        )
    }
}
