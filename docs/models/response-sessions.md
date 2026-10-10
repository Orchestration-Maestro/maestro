# Response-session requests

The dedicated provider lives in
`maestro_models::providers::response_sessions::openai_codex_responses`.
`stream_openai_codex_responses` returns a stream whose setup failures become error
updates. `stream_simple_openai_codex_responses` can return a missing-key error
before creating a stream. The two option/debug records are also exported at the
crate root; this provider is not yet part of generic protocol registration.

Missing transport selects `auto` for both dispatch and cached-context selection.
Explicit `sse` bypasses sockets. Otherwise, a session's sticky fallback selects
SSE; a socket transport failure falls back only before the first selected event.
After start, that failure ends the stream without replaying the response.
API/protocol failures and cancellation do not trigger fallback.

The invocation uses independently running request work and shared partial-message
handles. Native callers supply a running Tokio runtime. Payload hooks run during
preparation before headers or transport are processed.

`get_openai_codex_web_socket_debug_stats` returns an independent snapshot.
`reset_openai_codex_web_socket_debug_stats` clears counters and fallback state,
not connections. `close_openai_codex_web_socket_sessions` closes connections,
not counters or fallback state. An absent or empty control identity selects all
sessions. Public invocation registers socket cleanup with
[`cleanup_session_resources`](https://docs.rs/maestro-models/latest/maestro_models/fn.cleanup_session_resources.html).

Native User-Agent identity uses the operating system's release and the compile
target's platform/architecture. Request IDs are random v4 UUIDs unless a nonempty
session supplies the identity. Browser imports exclude native socket libraries;
browser socket unavailability follows the pre-start fallback path.

Preparation resolves a nonempty explicit key before the provider environment
key, extracts the account claim, converts the conversation, awaits the payload
hook once and validates headers. The retained payload-hook result is the SSE wire
body. See [response conversion](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/responses.md) for history and tool conversion
and [JSON records](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/records.md) for the shared record types.

Initial model header names combine case-insensitively in encounter order:
cookies use `; ` and other fields use `, `. Additional headers replace earlier values. Each incoming value is validated
before it can be overwritten. Provider authentication, host identity and SSE
fields are applied last. A nonempty session replaces both affinity headers;
an absent or empty session leaves caller-supplied affinity headers intact.

Run the controlled request witnesses without live credentials:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo test --locked -p maestro-models response_sessions
```

An explicitly requested reasoning effort uses the descriptor's mapping when
non-null, including an empty mapped spelling; otherwise it retains the requested
effort (`Off` sends `none`). Summary defaults to `auto` only when effort is
requested. No descriptor reasoning-capability check restricts raw effort.

The internal SSE invocation observes each actual response before checking its
status or body. Setup permits four attempts, with scoped waits of 1,000, 2,000
and 4,000 milliseconds. A known nonretryable status returns its failure without
entering the network retry branch. Common retry/timeout settings do not replace
this provider's setup policy.

Usable success bodies publish `start`. Provider event selection trims each data
field, skips empty data and exact `[DONE]`, and normalizes terminal event names
before delegating content reduction to the [shared response reducer](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/responses.md).
The transport operation delegates final done/error publication to the dedicated
invocation caller.

## Native socket request

The internal socket operation sends one request over a native connection.
Without a session identifier it opens a fresh connection and releases it after
the request. Socket headers derive
from the validated request headers: accept and content type are removed, the
`openai-beta` field selects the socket beta, and the supplied request identifier
replaces both affinity fields. Header bytes keep their one-byte-per-character
form. The endpoint composes like the SSE endpoint, then `http` and `https`
become `ws` and `wss`; any restriction of the native connector applies when it
connects. The socket makes one attempt and uses none of the SSE setup retry
settings. Browser targets exclude this operation.

The request text is the retained payload with `type: "response.create"` as the
default `type`; an edited `type` or `store` from the payload hook wins, and the
hook runs once during preparation. Member order and number spelling follow the
shared compact JSON writer. Empty text messages are ignored, and event selection
is shared with the SSE path, but socket text is neither trimmed nor filtered for
`[DONE]`. Binary messages are decoded by the shared HTTP text decoder.

`start` is published immediately before the first selected event. A terminal
event completes the operation without waiting for the peer to close, and later
messages are not reduced into the completed operation. Failures keep their category: server errors are API
failures, unreadable messages are protocol failures, and a close or transport
problem is a transport failure. A close reports `WebSocket closed` with its
numeric code and reason, and an empty reason for code 1009 reads
`message too big`. Once the endpoint resolves and the upgrade starts,
cancellation reports `Request was aborted`; an earlier preparation or endpoint
failure is reported instead. The connector rejects an endpoint with a fragment
or a scheme other than `ws` and `wss` before connecting. Dropping the operation
releases the connection.

## Session connections

With a session identifier, a healthy connection stays cached for 300,000
milliseconds after its request and serves the next request whose endpoint and
effective headers are equal (header names compare case-insensitively); a request
with different values replaces it. While another operation holds the session's
connection, a new request uses an uncached connection and releases it afterwards.
An idle connection answers pings and discards application messages without
reducing them, and leaves the cache when the peer closes it, the period elapses
(close reason `idle_timeout`) or the session is closed. A request that fails
on the session's cached connection, whether in the exchange (including a
cancellation before it completes) or in retaining its context, closes that
connection and clears the cached entry's context; a request cancelled after it
completed closes its connection and keeps the context. A request on an uncached
connection (the session's connection was busy) and a failure while preparing
the request or connecting leave the session's context intact.

The `websocket-cached` and `auto` transports keep, per cached connection, the
body, response identifier and contributed input items of the most recent
completion that has a nonempty response identifier; a completion without one
leaves the earlier baseline unchanged. The next request sends only the input items that follow
those, with `previous_response_id`, when every other member is equal and the
earlier input and response items are a prefix of the new input; otherwise it
sends the full request and the context is dropped. Each selected request is
counted in the session's debug statistics.

Closing a session (one or all) removes it from the cache and closes its
connection with reason `debug_close`; statistics and fallback state stay. A
request in flight sends that close, keeps reading, reduces the messages the
peer still delivers, and ends with its own result or the peer's close, so a
queued terminal response still completes it. A connection released without a
close request reports reason `done`.


## Controlled invocation

This example uses a synthetic account claim and an in-memory HTTP response;
it contacts no service.

```rust
# #[cfg(not(target_arch = "wasm32"))]
# fn main() -> Result<(), Box<dyn std::error::Error>> {
use base64::Engine as _;
use maestro_models::{Context, HttpResponse, Model, StreamOptions, StopReason, Transport};
use maestro_models::providers::response_sessions::openai_codex_responses::{
    OpenAICodexResponsesOptions, stream_openai_codex_responses,
};
use std::sync::Arc;

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "controlled", "name": "Controlled", "api": "openai-codex-responses",
    "provider": "fixture-no-key", "baseUrl": "https://example.test",
    "reasoning": false, "input": ["text"],
    "cost": {"input": 1, "output": 2, "cacheRead": 0, "cacheWrite": 0},
    "contextWindow": 1000, "maxTokens": 100
}))?;
let claims = serde_json::to_vec(&serde_json::json!({
    "https://api.openai.com/auth": {"chatgpt_account_id": "controlled"}
}))?;
let key = ["header", &base64::engine::general_purpose::STANDARD.encode(claims),
    "signature"].join(".");
let options = OpenAICodexResponsesOptions {
    common: StreamOptions {
        api_key: Some(key), transport: Some(Transport::Sse),
        fetch: Some(Arc::new(|_| Box::pin(async {
            Ok(HttpResponse {
                status: 200, status_text: "OK".into(), headers: Default::default(),
                body: Box::pin(futures_util::stream::iter([Ok(
                    b"data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\n\n".to_vec()
                )])),
            })
        }))),
        ..Default::default()
    },
    ..Default::default()
};
let context = Context { system_prompt: None, messages: vec![], tools: None };
let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
runtime.block_on(async {
    let events = stream_openai_codex_responses(model, context, Some(options));
    assert_eq!(events.result().await.read().unwrap().stop_reason, StopReason::Stop);
});
# Ok(())
# }
# #[cfg(target_arch = "wasm32")]
# fn main() {}
```
