# Configurable chat connection

`ChatConnection` implements the existing `Provider` interface. Register it with
`Models`; existing `Models::stream` and `Models::complete` callers do not change.
Completion drains exactly the same stream. Different per-model dialects use
separately registered connection instances. No catalog or provider-name roster
is bundled.

## Configuration

Construct `NativeHttpTransport::new()` under a caller-owned Tokio runtime, then
pass `Arc<dyn ChatTransport>` and an explicit `ChatDialect` to
`ChatConnection::new`. Construction creates a client, not a runtime or request.
A scripted transport can replace native HTTP without editing model callers.

Set the captured model's protocol to `chat-completions`, operation to `chat`,
and endpoint to the supplied HTTP(S) **base URL**, for example
`https://service.example/v1`. The adapter removes only one joining slash,
appends `/chat/completions`, then parses that entire URL. It does not resolve a
filesystem path or replace `/v1`. Dot segments and percent escapes follow URL
parsing. A query or fragment is not stripped: the appended suffix becomes part
of that query or fragment. Supply a query-free base URL for an ordinary route.

Every dialect field is required; there is no implicit dialect default:

| Declaration | Wire behavior |
| --- | --- |
| store, usage_in_stream | store:false, stream_options.include_usage |
| developer_role, output_field | reasoning instruction role; max_tokens or max_completion_tokens |
| tool_result_name, assistant_after_tool_result | result name; bridge before a following user message |
| thinking_as_text, empty_reasoning_content | readable replay; required empty reasoning field |
| thinking_format | effort, nested effort, toggle, template toggle or typed toggle |
| provider_routing, provider_options | verbatim provider and providerOptions objects |
| tool_stream, strict_tools | declared nonempty-tool streaming; function.strict:false |
| cache_control | ephemeral markers on eligible instructions, tools and text |
| prompt_cache_key, session_affinity_headers, long_cache_retention | explicit session key, affinity headers and long retention |
| truncate_plain_call_ids | forty-UTF-16-unit rule for foreign plain IDs |
| auth_header, auth_prefix | supplied credential header and literal prefix |

Declare optional request support in the model's existing
`RequestCapabilities`. `StreamOptions.tool_choice` is omitted unless
`RequestCapabilities.tool_choice` is true. Thinking effort is resolved once by
Models. Enabled token-budget requests are unsupported by this connection.

For example, set `auth_header` to `"authorization"` and `auth_prefix` to
`"Bearer "`, then supply
`RequestAuth::Secret { secret: SecretString::new(supplied), source: None }`
in `StreamOptions.auth`. An explicitly configured secret-free endpoint uses
`RequestAuth::ConfiguredWithoutSecret { source: None }`. There is no ambient
credential discovery or token exchange. Literal provider/model/request headers
are case-insensitive overlays; request headers win over generated auth and
affinity defaults. Invalid headers fail before sending.

Caching has no environment default. Declare supported preferences and session
affinity, then supply `cache_preference: Some("short".into())` or `"long"`, and
an optional nonempty session. `"none"` or absence adds no cache fields or affinity
headers. Declared long retention uses `"24h"`; ephemeral long markers use
`"1h"` only when supported.

## Stream termination and safety

A recognized stop/end, length or tool_calls/function_call finish reason plus
clean EOF or exact `[DONE]` is required. A marker alone is incomplete. Malformed
JSON, UTF-8 or an incomplete final SSE frame fails even after a finish reason.
Usage-only trailers before the boundary are retained. Tool arguments remain
unavailable until strict object parsing at validated stream end. Owned earlier
snapshots never change when tool identity or replay metadata arrives later.

The adapter normalizes raw cache overlaps into flat usage categories; shared
accounting alone derives totals and captured-rate prices. Successful output
already includes reasoning tokens. No context-window heuristic invents failures.
Only typed categories and fixed safe messages escape error classification.
Requests, headers, endpoint, dialect and transport must not be logged. Context
content remains ordinary caller-supplied data, not an error-redaction surface.

## Time, retries and cancellation

- Response-header timeout: **600,000 ms per attempt**. `timeout_ms` overrides it;
  zero never polls network setup and fails immediately with `SetupTimeout`.
- Additional setup retries: **2**, at most three sends. `max_retries: Some(0)`
  disables retries. Connection/setup-timeout and HTTP 408/409/429/5xx are eligible;
  literal `x-should-retry: true` or `false` overrides status eligibility.
- Delay precedence: valid nonnegative complete `retry-after-ms` (zero included),
  then numeric Retry-After seconds or an HTTP date, then
  `min(500 * 2^i, 8000) * (1-u)` ms, with `i=0` initially and `u` in `[0,0.25)`.
- Server delays are **uncapped**. Malformed, nonfinite, negative or
  Duration-unrepresentable delays (including finite `1e300`) fall through to the
  next allowed header or fallback; they are never clamped. Fractional precision
  is preserved. Dates accept only IMF-fixdate, RFC850 and asctime in UTC, using
  the supplied clock and RFC850's fifty-year interpretation.
- There is **no body-idle or whole-operation deadline**, no hidden native protocol
  retry, and no detached request task. Authentication construction and successful
  response-body failures never retry. Cancellation wins readiness ties during
  setup, body reads and backoff; dropping the stream releases local work without
  claiming to undo remote effects.

Native HTTP preserves reqwest 0.13.5 full defaults: rustls on aws-lc-rs,
operating-system roots, HTTP/2, system proxy (HTTP_PROXY/HTTPS_PROXY/NO_PROXY),
and charset. It does not select another TLS backend or disable system proxies.

## Deterministic scalar adaptations

Replay blankness uses ECMAScript whitespace without trimming retained text.
Compact JSON uses binary64 number spelling and integer-index keys first, then
the existing shared Map iteration order. It cannot recover insertion order
already lost by that map. Foreign ID conversion uses UTF-16 units, deterministic
sibling collision disambiguation and paired projected result IDs. Native Rust
strings cannot carry lone UTF-16 surrogates; malformed wire escapes fail safely.

## Registration example

```rust,no_run
use maestro_models::*;
use std::{collections::BTreeMap, sync::Arc};

# async fn example() -> Result<(), Failure> {
let dialect = ChatDialect {
    store: false,
    developer_role: false,
    usage_in_stream: true,
    output_field: ChatOutputField::MaxCompletionTokens,
    tool_result_name: false,
    assistant_after_tool_result: false,
    thinking_as_text: false,
    empty_reasoning_content: false,
    thinking_format: Some(ChatThinkingFormat::Effort),
    provider_routing: None,
    provider_options: None,
    tool_stream: false,
    strict_tools: false,
    cache_control: None,
    prompt_cache_key: false,
    session_affinity_headers: false,
    long_cache_retention: false,
    truncate_plain_call_ids: false,
    auth_header: "authorization".into(),
    auth_prefix: "Bearer ".into(),
};
let transport: Arc<dyn ChatTransport> = Arc::new(NativeHttpTransport::new()?);
let connection = Arc::new(ChatConnection::new(transport, dialect));
let model = Model::custom(
    ModelIdentity {
        provider: "configured".into(),
        model: "selected".into(),
        operation: "chat".into(),
    },
    "chat-completions".into(),
    "https://service.example/v1".into(),
);
let mut models = Models::new(Arc::new(|| 0));
models.register(model.clone(), connection)?;
let options = StreamOptions {
    auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
    headers: BTreeMap::new(),
    max_retries: Some(0),
    ..StreamOptions::default()
};
let context = Context {
    system_prompt: Some("Answer briefly.".into()),
    messages: Vec::new(),
    tools: Vec::new(),
};
let response = models.complete(model, context, options).await;
assert_eq!(response.failure, None);
# Ok(())
# }
```
