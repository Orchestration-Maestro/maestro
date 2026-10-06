# Supplied model invocation

A Model is a caller-supplied descriptor. Register an ApiProvider for its api, then call stream or stream_simple. Invocation does not require a model catalog entry.

The raw and simple entry points forward their options to different adapter callbacks. This layer does not select authentication, project messages, merge headers, or apply provider defaults.

AssistantMessageEventStream keeps queued events and one independently observable result. A terminal event settles result observation without consuming queued events. Earlier event handles observe later mutations of their shared message.

Dropping an iterator, result future, or observing handle does not cancel producer-owned work. Calling end without a result closes iteration but leaves result observation pending; a later explicit result can settle it.

Unknown APIs and adapter setup errors are invocation failures. An error event instead resolves the stream result to its error assistant message. Session resource cleanup runs the live callback set and reports all collected errors after the sweep.


```rust
use maestro_models::*;
use std::sync::{Arc, RwLock};
let descriptor = Model {
    id: "synthetic".into(), name: "Synthetic".into(), api: "synthetic".into(),
    provider: "local".into(), base_url: String::new(), reasoning: false,
    thinking_level_map: None, input: vec!["text".into()],
    cost: TokenRates { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 },
    context_window: 0.0, max_tokens: 0.0, headers: None, compat: None,
};
fn produce(model: Model) -> Result<AssistantMessageEventStream, ThrownValue> {
    let message = Arc::new(RwLock::new(AssistantMessage {
        content: vec![], api: model.api, provider: model.provider, model: model.id,
        response_model: None, response_id: None, diagnostics: None,
        usage: Usage { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0,
            total_tokens: 0.0, cost: UsageCost { input: 0.0, output: 0.0,
                cache_read: 0.0, cache_write: 0.0, total: 0.0 } },
        stop_reason: StopReason::Stop, error_message: None, timestamp: 0.0,
    }));
    let stream = create_assistant_message_event_stream();
    stream.push(AssistantMessageEvent::Start { partial: message.clone() })?;
    stream.push(AssistantMessageEvent::Done { reason: StopReason::Stop, message })?;
    Ok(stream)
}
register_api_provider(ApiProvider {
    api: descriptor.api.clone(),
    stream: Arc::new(|model, _, _| produce(model)),
    stream_simple: Arc::new(|model, _, _| produce(model)),
}, None);
let stream = stream(descriptor, Context { system_prompt: None, messages: vec![], tools: None }, None).unwrap();
# fn ready<F: std::future::Future>(future: F) -> F::Output {
#     let mut future = std::pin::pin!(future);
#     let mut context = std::task::Context::from_waker(std::task::Waker::noop());
#     match future.as_mut().poll(&mut context) {
#         std::task::Poll::Ready(value) => value,
#         std::task::Poll::Pending => panic!("controlled producer already settled"),
#     }
# }
let result = ready(stream.result());
assert_eq!(result.read().unwrap().stop_reason, StopReason::Stop);
let mut queued_events = stream.iter();
assert!(matches!(ready(queued_events.next()), Some(AssistantMessageEvent::Start { .. })));
assert!(matches!(ready(queued_events.next()), Some(AssistantMessageEvent::Done { .. })));
assert!(Arc::ptr_eq(&result, &ready(stream.result())));
clear_api_providers();
```
