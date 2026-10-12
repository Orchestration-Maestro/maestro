# Proxy stream client

`stream_proxy(model, context, ProxyStreamOptions, spawn)` sends one request to a
caller-chosen proxy and returns the model's [assistant event stream](records.md#stream-ownership)
immediately. It is a replacement for model dispatch (`Agent`'s `stream_fn`); no proxy
server or hosted service is part of Maestro.

`ProxyStreamOptions` holds the proxy address (`proxy_url`), the bearer credential
(`auth_token`) and the ordinary `SimpleStreamOptions`. The credential is sent only in
the `Authorization` header. The replacement transport and the cancellation signal are
the options' `common.fetch` and `common.signal`; an absent transport selects
`default_fetch`. `spawn` receives the one producer future before `stream_proxy`
returns. Dropping the stream's observers does not stop the producer. Targets are native and
`wasm32`.

## Request

`POST {proxy_url}/api/stream` — the address is concatenated literally, so a trailing
slash yields `//api/stream`. The headers are `Authorization: Bearer <auth_token>` and
`Content-Type: application/json`. The body is `model`, `context` and `options`, written
with the model-owned canonical JSON writer (array-index keys first, ECMAScript number
spelling). `options` holds, in this order, `temperature`, `maxTokens`, `reasoning`,
`cacheRetention`, `sessionId`, `headers`, `metadata`, `transport`, `thinkingBudgets`
and `maxRetryDelayMs`. Absent values are omitted; explicitly empty values are kept.
Credentials, callbacks, the signal, the timeout and the retry count never reach the body, and
the options' `headers` are provider headers, separate from the two request headers.

## Response framing

Bytes are split on LF only. A line counts when it starts with `data: `; its payload is
trimmed of U+FEFF and Unicode whitespace other than U+0085 and then parsed. Empty and
non-matching lines are ignored. A byte-order mark is dropped only at the start of the
stream. Invalid UTF-8 becomes U+FFFD. CR is not a separator, `[DONE]` is not special,
data lines are not joined, and an unterminated final line is never processed.

## Compact events

The twelve wire forms are `start`, `text_start|delta|end`, `thinking_start|delta|end`,
`toolcall_start|delta|end`, `done` and `error`, decoded into `ProxyAssistantMessageEvent`
(`contentIndex`, `contentSignature`, `toolName`, `errorMessage` keep their wire names).
A payload is parsed as a whole first, so the last value of a repeated member wins at
every depth; unknown members are ignored.

Every event is applied to one shared assistant message that is also the stream's final
result. It starts with the selected `api`, `provider` and model id, empty content, stop
reason `stop`, zero usage and the current time. Starts replace the block at their index
(a new index must be the next free one); deltas append; ends set or clear the signature
and carry the accumulated text. Tool arguments are reparsed on every delta with the
model-owned partial JSON parser; a restart forgets earlier argument text, and a `toolcall_end` for an
index that holds another kind of block publishes nothing. `done` and `error` copy the supplied
stop reason, usage, cost and message without repricing.

A discriminant that is unknown, or missing (shown as `undefined`), prints
`Unhandled proxy event type: <type>` to standard error on native targets and the event is
skipped.

## Failures

Every failure marks the shared message `error` (or `aborted` when the signal is
aborted), stores the text and publishes one terminal event; received content and usage
are kept.

| Cause | Text |
| --- | --- |
| Status outside 200–299 | `Proxy error: <status> <status text>`, or `Proxy error: <error>` when the JSON body holds a nonempty string `error` |
| Abort | `Request aborted by user` |
| Content update for the wrong block kind | `Received text_delta for non-text content`, `text_end`, `thinking_delta`, `thinking_end` (`non-thinking`) or `toolcall_delta` (`non-toolCall`) |
| Body ends before `done` or `error` | `Proxy stream ended before a terminal event` |
| Anything else | the transport, JSON or decoding diagnostic |

An abort wins over the premature-end text. Reading continues after a terminal event: later
input still edits the retained message (and a later failure or abort rewrites it), but only
the first terminal event is ever delivered. A discriminant with a wrongly typed field fails
with the decoder's diagnostic; a block index that skips ahead of the next free slot fails
instead of leaving a gap.

There is no retry, timeout, buffer limit or URL normalization. Aborting drops the open
request or body; it does not undo work already done by the server. Browser visibility of
the unknown-event warning is tracked separately.
