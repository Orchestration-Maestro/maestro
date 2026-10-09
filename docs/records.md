# Model invocation records

`maestro-models` supplies typed conversations, model descriptors and invocation
options through replaceable registered protocol callbacks. An invocable model
need not belong to a catalog. Registration wraps both callbacks with a protocol
check. Replacing a protocol preserves its position but replaces its source owner;
source removal affects only registrations still owned by that source. Retained
handles remain usable after replacement, removal or clear.

`stream` and `stream_simple` invoke immediately and return setup errors directly.
`complete` and `complete_simple` also invoke immediately, but return setup errors
inside their futures. Options, hooks and cancellation signals reach the adapter
unchanged; adapters own payload policy, credentials, I/O and provider defaults.
A payload hook receives the submitted payload and returns the one to send, so it can
keep, edit or replace it; a response hook observes status and headers before the adapter
consumes a response body. Both receive the invocation's model, shared and read-only.

The [offline model catalog](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/catalog.md) provides owned embedded descriptors,
exact lookup, thinking-level selection, identity comparison and flat-rate costs.

The [request authentication guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/request-authentication.md)
explains explicit environment-key discovery and ambient configuration.

The [model option helpers](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/model-options.md)
copy common settings, adjust thinking budgets and classify context overflow.

The [chat-completion guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/chat-completions.md)
covers direct streamed requests, header and cache policy, retries and the shared
Cloudflare and Copilot helpers.

The [response-event guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/responses.md)
covers history and tool conversion and the reduction of streamed response events.

The [OAuth authorization guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/oauth.md)
covers data records, secure proof keys, escaped callback pages and subscription
login and refresh.

The [message-protocol guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/messages.md)
covers direct streamed message requests with key authentication, their payload, cache and
header policy, the event framing and repair, and replaceable clients.

See [Tool arguments](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/arguments.md) for shared JSON completion and repair.

The [conversation projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/conversation-projection.md)
produces replay-safe request histories without editing stored messages. Providers
supply ID normalization; projection owns content conversion and missing results.

## Controlled registered invocation

```rust
use maestro_models::{
    ApiProvider, AssistantMessage, AssistantMessageEvent, Context, DoneReason,
    Model, clear_api_providers, complete, create_assistant_message_event_stream,
    register_api_provider,
};
use std::{future::Future, sync::{Arc, RwLock}, task::{Context as PollContext, Poll, Waker}};

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "controlled", "name": "Controlled model", "api": "fixture",
    "provider": "fixture", "baseUrl": "https://fixture.invalid",
    "reasoning": false, "input": ["text"], "contextWindow": 100,
    "maxTokens": 10,
    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
}))?;
let message: AssistantMessage = serde_json::from_value(serde_json::json!({
    "role": "assistant", "content": [{"type": "text", "text": "ready"}],
    "api": "fixture", "provider": "fixture", "model": "controlled",
    "usage": {"input": 0, "output": 1, "cacheRead": 0, "cacheWrite": 0,
        "totalTokens": 1,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
    "stopReason": "stop", "timestamp": 1
}))?;
let message = Arc::new(RwLock::new(message));
let producer = create_assistant_message_event_stream();
producer.push(AssistantMessageEvent::Done {
    reason: DoneReason::Stop, message: Arc::clone(&message),
});
let raw = producer.clone();
let simple = producer.clone();
register_api_provider(ApiProvider {
    api: "fixture".into(),
    stream: Arc::new(move |_, _, _| Ok(raw.clone())),
    stream_simple: Arc::new(move |_, _, _| Ok(simple.clone())),
}, Some("controlled-example".into()));
let completion = complete(model, Context {
    system_prompt: None, messages: vec![], tools: None,
}, None);
clear_api_providers(); // The admitted invocation retains its producer.
let mut context = PollContext::from_waker(Waker::noop());
let mut completion = std::pin::pin!(completion);
assert!(matches!(completion.as_mut().poll(&mut context), Poll::Ready(Ok(_))));
let mut event = std::pin::pin!(producer.next());
assert!(matches!(event.as_mut().poll(&mut context),
    Poll::Ready(Some(AssistantMessageEvent::Done { .. }))));
# Ok::<(), serde_json::Error>(())
```

## Stream ownership

`EventStream` is a producer-owned FIFO. Readers compete for events; the first
terminal result is independently and repeatedly observable. Assistant updates
and terminal observations retain the same live message handle, not snapshots.
Dropping a reader or result observer does not cancel production. A terminal
event reaches one pending reader and wakes all remaining readers to EOF. End
retains queued events; an absent result stays pending until an explicit result
arrives. The first supplied result is never replaced.

Classification and extraction run outside stream locks. Terminal admission is
reserved before extraction, so an extractor's reentrant pushes are ignored.
Readers and result observers wait for publication. Caller wakers, retained-result
clones and callback destruction also execute outside ownership locks.

`Cancellation` starts un-aborted. Clones share the same signal; `abort` is
idempotent and wakes every cancellation observer. The caller decides which
producer observes a signal; unrelated streams are not automatically stopped.

## Cleanup and diagnostics

Session cleanup registrations deduplicate callback identity and preserve order.
Removal is explicit and idempotent; dropping a removal handle does not unsubscribe.
Traversal is live: added callbacks run in that pass, while unvisited removed
callbacks are skipped. Nested cleanup is permitted. Every callback is attempted;
failures are returned in encounter order with `Failed to cleanup session resources`.

Diagnostics retain supplied name, message, stack and textual or numeric code;
no cause chain is inferred. An empty message falls back to the supplied name.
Already-formatted thrown text is retained unchanged. Created diagnostics use
current epoch milliseconds and append without removing prior entries.

## Wire records and helpers

Messages and content serialize with their fixed role/type tags, even when used
alone. Tagged unions decode by their role/type. Optional absent fields are omitted;
explicit null is retained for supported thinking/routing fields and open tool
details. Model compatibility decoding uses the protocol identifier. Signature
strings remain opaque; the separate signature metadata record accepts version 1.

`short_hash` mixes supplied UTF-16 units with wrapping unsigned arithmetic and
emits two concatenated lower-case base-36 words. It is not a cryptographic hash.
`headers_to_record` copies an already-normalized name/value iterator; an exact
repeated key replaces its previous value, without parsing or HTTP normalization.
`string_enum` retains supplied order and nonempty description/default strings
without trimming, list validation or default-membership validation.

## Scripted models

The opt-in faux provider supplies offline queued responses, async factories and
ordered cancellable events through the ordinary invocation interface. See the
[scripted model guide](models/faux.md) for a two-turn tool flow and defaults.
