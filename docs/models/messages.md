# Message protocol

`maestro-models` sends message-protocol requests directly and streams the answer as shared
assistant updates. `stream_anthropic(model, context, options)` takes `AnthropicOptions`: the
common `StreamOptions`, a typed `ToolChoice`, the interleaved-reasoning switch and an optional
client. It returns the call's `AssistantMessageEventStream`, and every failure the call detects,
including a missing key, ends that stream with an error update instead of failing the call.

This interface covers key-authenticated requests. Subscription, account and gateway
authorization, thinking options and the simple-options entry point are not part of it yet.

On native targets the call must run inside a Tokio runtime. A request through the shared HTTP sender
needs the runtime's time driver (`enable_time`, or `enable_all`) for its setup timeout and retry
delays, and the default HTTP client also needs the I/O driver; an injected client reads the body
without timers, so a call that uses one needs neither driver. The request work is owned by the
runtime, so dropping the returned stream does not cancel it and `result()` still resolves. Called
outside a runtime, the stream ends at once with `Streaming requires a running Tokio runtime.`
Browser targets use local futures.

## Key and client

The request is authenticated with the explicit `api_key`, including an explicitly empty one,
otherwise `get_env_api_key(provider)`, otherwise the empty string. The environment variable
`ANTHROPIC_AUTH_TOKEN`, trimmed of script whitespace (the byte-order mark counts, U+0085 does
not), adds an `authorization: Bearer` header when it is not empty.

`AnthropicOptions::client` replaces the key, the endpoint and the HTTP transport with one
function from the payload and an `AnthropicRequestOptions` to an `HttpResponse`. The client
receives the payload after `on_payload` ran and `stream` was set to `true`; the options carry
exactly the signal, timeout and retry count the call was given, each absent when it was not
given. The client owns authentication, retries and status handling: a response it returns is
read as an event stream whatever its status, and a failure it reports ends the call with the
failure's `message` as it is, even when that is empty. Payload construction, `on_payload`,
`on_response`, cache markers and the reduction of the answer are the same as without a client.

## Request

The endpoint is the model's base URL with `/v1/messages` appended (the base `https://api.anthropic.com`
when the model's is empty); a trailing slash on the base is not doubled.

Headers are layered, later layers replacing earlier ones case-insensitively: the protocol defaults
(`accept`, `anthropic-version: 2023-06-01`, `anthropic-dangerous-direct-browser-access: true`),
the key as `x-api-key`, the ambient bearer token, `anthropic-beta`, the model's headers and the
caller's headers. Names are sent in lowercase. The body is JSON, so `content-type:
application/json` is set last and replaces even a caller's. A request needs a nonempty
`x-api-key` or `authorization` once the layers are merged and edge whitespace is removed;
otherwise the stream fails after `on_payload` has run and before any request is sent, with
`Could not resolve authentication method. Expected either apiKey or authToken to be set. Or for one of the "X-Api-Key" or "Authorization" headers to be explicitly omitted`.
A caller's `authorization` or `x-api-key` header can supply the authentication.

`anthropic-beta` names `interleaved-thinking-2025-05-14` unless the caller sets
`interleaved_thinking` to `false` or the model ID contains `opus-4-6`, `opus-4.6`, `opus-4-7`,
`opus-4.7`, `sonnet-4-6` or `sonnet-4.6`, and `fine-grained-tool-streaming-2025-05-14` when
tools are declared and the model's compatibility turns eager input streaming off. With eager
input streaming (the default) each tool carries `eager_input_streaming: true` instead.

The payload holds `model`, `messages`, `max_tokens`, `stream`, then `system`, `temperature`,
`tools`, `metadata` and `tool_choice` where they apply. `max_tokens` is the caller's value
unless it is zero or not a number, otherwise a third of the model's, truncated toward zero. An
infinite limit, and a temperature that is infinite or not a number, are written as `null`;
negative zero is written as `0`. `metadata` carries `user_id` only when the caller's metadata
holds it as text. `ToolChoice::Auto` and `None` are sent as `auto` and `none`, `Required` as
`any`, and `Function` as a `tool` choice naming the function. A tool declares its name,
description and the `properties` (default `{}`) and `required` (default `[]`) of its schema as
an object schema; other schema members are not sent.

`on_payload` receives the payload as JSON before it is sent and returns the payload to send:
the one it received, edited or not, or a new one. `stream` is then set to `true`; a returned
value that is not an object becomes `{"stream": true}`. `on_response` receives the status and
headers of the accepted response before the `start` update; it is not called for responses that
are retried. Both hooks also receive the one model the request holds, shared and read-only. A
hook error ends the stream with the error's message as it is.

## History

The history is projected for the model first (see the conversation projection guide). The tool call
identifiers of an assistant turn from another provider, API or model are normalized: every UTF-16
unit outside `[A-Za-z0-9_-]` becomes one `_` and the result is cut to 64 characters, and the results
of those calls follow. A turn from the same provider, API and model keeps its identifiers. In user
and assistant messages, text, text blocks and unredacted reasoning that are blank are dropped, and a
message left without content is omitted; spaces, line breaks, the no-break space and the byte-order
mark count as blank, U+0085 and U+200B do not, and kept text is never trimmed. Redacted reasoning is
kept whatever its text, but only when it comes from the same provider, API and model; other models
never receive it. Images are sent with their type and data when the model accepts images, and
otherwise the projection replaces them with placeholder text. Reasoning with a nonblank signature,
which only the same provider, API and model can carry, is replayed signed; other reasoning is
replayed as plain text; redacted reasoning is replayed as its opaque payload. Consecutive tool
results become one user message, and blank text blocks in a result are kept. A tool result of text
blocks only is their text joined by line feeds; one that holds an image is sent as blocks in order,
led by `(see attached image)` when it has no text block.

## Cache

Retention is the explicit option, otherwise `long` when `MAESTRO_CACHE_RETENTION` is exactly
`long`, otherwise `short`. `none` sends no cache markers. Otherwise a marker goes on the system
prompt, on the last tool and on the last block of the final message when that message is the
user's; it never goes back past a final assistant message. With `long` retention the marker
carries a one-hour lifetime unless the model's compatibility turns long retention off; the
endpoint's host plays no part.

## Stream

The body is read as server-sent events with this protocol's own framing; no event-stream library is
involved. Lines end at `\r\n`, `\r` or `\n`: a carriage return ends its line at once and a line feed
right after it, even in the next chunk, belongs to the same ending. Bytes that are not valid UTF-8
become U+FFFD, a character cut by the end of a chunk is completed by the next one (or becomes U+FFFD
when the body ends there), and one byte-order mark is dropped from the start of the stream (a second
one stays). A blank line dispatches the event collected so far when it has a nonempty `event` name
or a `data` line, and the end of the body dispatches a last line and event. One space after the
colon of a field is dropped, `data` lines are joined by `\n`, the latest `event` field wins, and
comments and other fields are ignored but kept for diagnostics.

Only events named `message_start`, `message_delta`, `message_stop`, `content_block_start`,
`content_block_delta` and `content_block_stop` are read as JSON; an `error` event ends the call with
its data as the failure text, and any other event is ignored. The JSON is read as written and read
again with its string literals repaired (raw control characters and invalid escapes) only when that
fails; the event's own `type` decides how it is reduced, and a root that is neither an object nor
`null`, or an object whose `type` is missing, not text or not a known event type, is ignored. A
`null` root and data that is not JSON fail with `Could not parse Anthropic SSE event {event}:
{cause}; data={data}; raw={lines}`, where the cause is the reader's own and the lines since the
previous event are joined by a literal `\n`. A known event that lacks a member it needs fails with
the reader's own text. The usage of `message_start` and of `message_delta` is read where the
reduction needs it, so `message_start` has recorded the response ID, and a `message_delta` has
rejected an unhandled stop reason, before a missing usage fails the call. Members that repeat keep
their last value, and every number is the double it rounds to, so a number that no field reads
cannot fail an event.

Content blocks are addressed by the position the service gives them, which is kept apart from their
position in the message content. Positions match as strict equality matches: a missing position only
a missing one, `null` only `null`, numbers as doubles (`0` and `-0` match), text only equal text,
`true` and `false` only themselves, and an array or object nothing at all. Text positions retain
unpaired UTF-16 units, so equivalent escaped spellings match. Updates for another kind of block,
an unknown or a closed position are ignored, and so is an update that lacks its text, reasoning,
signature or argument fragment. Signature fragments of a reasoning block accumulate without an
update. A redacted reasoning block reads `[Reasoning redacted]` and keeps its opaque payload and the
redaction flag. A tool call opens with the arguments it carries when they are an object that nests
at most 127 containers; argument fragments replace them with the arguments the fragments so far
spell, read as in [tool arguments](arguments.md): numbers round to doubles (an overflow is `null`),
and a value nested past 127 containers, like any non-object, becomes an empty object. A call that
received no fragment keeps its opening arguments when it closes. Usage starts from the counts of
`message_start`, missing ones counting as zero, and later reports replace only the counts they
supply; input, output, cache reads and cache writes are summed and priced at the model's rates.
`end_turn`, `pause_turn` and `stop_sequence` end with `stop`, `max_tokens` with `length` and
`tool_use` with `toolUse`; `refusal` and `sensitive` fail with `An unknown error occurred`, and any
other nonempty reason with `Unhandled stop reason: {reason}`.

The stream is complete when `message_stop` arrives: the body is dropped there without reading
the rest. A body that ends first fails with `Anthropic stream ended before message_stop`,
whether or not a message started. The shared message holds only message content, never reading
state, whether the call ends in success or failure.

Every update and the final message share the same handle, so an earlier update shows later
changes of the same call. On failure the partial content and usage are kept and the last
update is an `error` update, whose reason is `aborted` when the signal was aborted.

## Cancellation

An aborted signal is checked before each read of the body; one that is aborted while a read of an
injected client's body is pending does not interrupt that read, whose data still counts. The body of
the default transport is read against the signal, so an abort ends a pending read at once. Either
way the call ends with `Request was aborted` and keeps the usage and content it had. With the shared
sender, a signal aborted before the request is accepted ends the call with a single `error` update,
without a `start` update, carrying the sender's `Request was aborted.`

## Transport

Requests go through the shared HTTP sender: a replacement transport in `StreamOptions::fetch`,
retries of connection failures, timeouts and transient statuses, the default two retries and
600,000-millisecond setup timeout are those of the chat-completion guide. A non-success response
ends the call with `{status} {message}`, where the message is that of the parsed response body (the
body's own `message` when it is truthy, as text when it is text and otherwise as compact JSON; the
whole body as compact JSON when `message` is missing, `null`, `false`, zero or empty), `{status}
{body}` for a body that is not JSON or is `null`, `false`, zero or an empty JSON string, and
`{status} status code (no body)` for an empty body. If compact conversion of the selected `message`
or whole body fails, its detail is empty, leaving `{status} `. The shared
[full-value conversion contract](chat-completions.md#whole-value-conversion) owns those conversion bounds.

## Example

```rust
use std::sync::Arc;

use futures_util::stream;
use maestro_models::{
    AnthropicClient, AnthropicOptions, AssistantMessageEvent, Context, HttpResponse, Model,
    stream_anthropic,
};

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "controlled", "name": "Controlled", "api": "anthropic-messages",
    "provider": "controlled", "baseUrl": "https://example.invalid",
    "reasoning": false, "input": ["text"], "contextWindow": 1000, "maxTokens": 100,
    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
}))?;
let context: Context = serde_json::from_value(serde_json::json!({
    "messages": [{"role": "user", "content": "hello", "timestamp": 0}]
}))?;
let answer = concat!(
    "event: message_start\n",
    r#"data: {"type":"message_start","message":{"id":"m","usage":{"input_tokens":1}}}"#, "\n\n",
    "event: content_block_start\n",
    r#"data: {"type":"content_block_start","index":0,"content_block":{"type":"text"}}"#, "\n\n",
    "event: content_block_delta\n",
    r#"data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}"#, "\n\n",
    "event: message_stop\n",
    r#"data: {"type":"message_stop"}"#, "\n\n",
);
let client: AnthropicClient = Arc::new(move |payload, _| {
    assert_eq!(payload["stream"], true);
    Box::pin(std::future::ready(Ok(HttpResponse {
        status_text: String::new(),
        status: 200,
        headers: Default::default(),
        body: Box::pin(stream::iter([Ok(answer.as_bytes().to_vec())])),
    })))
});
let options = AnthropicOptions { client: Some(client), ..AnthropicOptions::default() };
let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
let text = runtime.block_on(async {
    let stream = stream_anthropic(model, context, Some(options));
    let mut text = String::new();
    while let Some(event) = stream.next().await {
        if let AssistantMessageEvent::TextDelta { delta, .. } = event {
            text.push_str(&delta);
        }
    }
    text
});
assert_eq!(text, "hi");
# Ok::<(), Box<dyn std::error::Error>>(())
```
