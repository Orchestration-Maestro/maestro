# Response-session requests

The internal response-session adapter prepares the account claim, request body
and effective SSE headers. It has no public invocation entry point yet.

Preparation resolves a nonempty explicit key before the provider environment
key, extracts the account claim, converts the conversation, awaits the payload
hook once and validates headers. The retained payload-hook result is the SSE wire
body. See [response conversion](responses.md) for history and tool conversion
and [JSON records](../records.md) for the shared record types.

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
before delegating content reduction to the [shared response reducer](responses.md).
This internal operation leaves final done/error publication and stream ending
to its caller; no public dispatch caller is delivered yet.

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
messages are not read. Failures keep their category: server errors are API
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
(close reason `idle_timeout`) or the session is closed. A failed request,
including one cancelled before it completes, closes its connection and clears
the session's context; a request cancelled after it completed closes its
connection and keeps the context.

The `websocket-cached` and `auto` transports keep, per cached connection, the
last completed request body, its response identifier and the input items the
response contributes. The next request sends only the input items that follow
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
