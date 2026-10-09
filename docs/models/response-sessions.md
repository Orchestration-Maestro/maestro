# Response-session requests

The internal response-session adapter prepares the account claim, request body
and effective SSE headers. It has no public invocation entry point yet.

Preparation resolves a nonempty explicit key before the provider environment
key, extracts the account claim, converts the conversation, awaits the payload
hook once and validates headers. The retained payload-hook result is the wire
body. See [response conversion](responses.md) for history and tool conversion
and [JSON records](../records.md) for the shared record types.

Initial model header names combine case-insensitively in encounter order.
Additional headers replace earlier values. Each incoming value is validated
before it can be overwritten. Provider authentication, host identity and SSE
fields are applied last. A nonempty session replaces both affinity headers;
an absent or empty session leaves caller-supplied affinity headers intact.

Run the controlled request witnesses without live credentials:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo test --locked -p maestro-models response_sessions
```
