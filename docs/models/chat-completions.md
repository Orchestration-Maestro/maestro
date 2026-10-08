# Chat completions

`maestro-models` sends chat-completion requests directly and streams the answer as
shared assistant updates. Two entry points start a call immediately and return its
`AssistantMessageEventStream`:

- `stream_openai_completions(model, context, options)` takes the full
  `OpenAICompletionsOptions`: the common `StreamOptions` plus a typed `ToolChoice`
  and a `reasoning_effort`. Every failure, including a missing key, ends the stream
  with an error update instead of failing the call.
- `stream_simple_openai_completions(model, context, options)` takes
  `SimpleStreamOptions`. It resolves a nonempty explicit key or the provider's
  environment key first and returns `No API key for provider: {provider}` at once,
  before any stream exists. It then applies `build_base_options`, clamps the
  requested thinking level to what the model supports (absent or `off` requests no
  reasoning) and forwards the typed `tool_choice`.

Neither entry point registers itself; registration is separate. On native targets
the call must run inside a Tokio runtime with time and I/O enabled; the request work
is owned by the runtime, so dropping the returned stream does not cancel it and
`result()` still resolves. Called outside a runtime, the stream ends at once with
`Streaming requires a running Tokio runtime.` Browser targets use local futures.

## Request

The endpoint is the model's base URL with `/chat/completions` appended: a trailing
slash on the base is not doubled, and the joined text is normalized as a URL
(`https://host/a/../v1` becomes `https://host/v1`). Cloudflare providers first
substitute `{NAME}` placeholders made of uppercase letters, digits and underscores
from the environment; an unset or empty variable fails with
`{NAME} is required for provider {provider} but is not set.` See
`providers::chat::cloudflare` for the four endpoint constants and
`is_cloudflare_provider`.

The key is the explicit `api_key`, then `get_env_api_key(provider)`, then
`OPENAI_API_KEY`; with none of them the call fails with
`OpenAI API key is required. Set OPENAI_API_KEY environment variable or pass it as an argument.`

Headers are layered, later layers replacing earlier ones case-insensitively, repeated
names inside one layer included: the account defaults `openai-organization` and
`openai-project` from the `OPENAI_ORG_ID` and `OPENAI_PROJECT_ID` environment
variables (surrounding whitespace removed; unset or blank variables send nothing),
the model's headers, the GitHub Copilot dynamic headers
(`providers::chat::github_copilot_headers`), session affinity headers
(`session_id`, `x-client-request-id`, `x-session-affinity`) when the model's
compatibility enables them and caching is on, then the caller's headers. Names are
sent in lowercase. The generated `authorization: Bearer {key}` sits below all layers;
for `cloudflare-ai-gateway` it is replaced by `cf-aig-authorization`, and an
`Authorization` header supplied in any layer (any casing) is kept as the upstream
credential.

Compatibility is detected from the provider and base URL, then overridden by
the model's own compatibility record: store and developer-role support, reasoning
effort, usage in streams, tool-result names, assistant messages after tool
results, reasoning replayed as text, `reasoning_content` on assistant messages, the
token-limit field, the reasoning convention (`openai`, `openrouter`, `deepseek`,
`zai`, `qwen`, `qwen-chat-template`), strict tool fields, tool streaming, cache
markers, session affinity and the one-hour cache lifetime. A zero or NaN token limit is
omitted; a zero temperature is sent. A NaN or infinite temperature and an infinite token
limit are sent as `null`, the way JSON text writes them. Requested reasoning is mapped
through the model's `thinking_level_map`; an unrequested level is never invented.
`OpenRouter` and gateway routing preferences are forwarded only for their own hosts.

Cache retention is the explicit option, otherwise `long` when
`MAESTRO_CACHE_RETENTION` is exactly `long`, otherwise `short`. `none` suppresses
prompt-cache and affinity fields. Message-style cache markers go on the first
instruction, the last tool and the last conversation text. `long` sends the `24h`
retention and the session key only where the compatibility supports long caching;
direct `OpenAI` URLs send the session key for any retention but `none`.

`on_payload` sees the payload as JSON before it is sent (the temperature and token limit
as floating-point numbers, or `null` when they are not finite); a returned value replaces
it (presence, not truthiness). `on_response` sees the status and headers of the
accepted response before the `start` update; it is not called for responses that are
retried. A hook error ends the stream with that error.

## History

`convert_messages` projects the history for the model, then writes wire messages.
Tool-call IDs from other models' turns that contain `|` are cut at it and every
character outside `[A-Za-z0-9_-]` becomes one `_` per UTF-16 unit, up to 40 units.
Other such IDs are kept, except for the `openai` provider, where they are cut to 40
UTF-16 units at a character boundary. Blank text and reasoning are dropped: spaces, line breaks and the
byte-order mark count as blank, U+0085 does not. Kept text is never trimmed. Instructions use the developer role when the model reasons and the endpoint
supports it. Tool results are grouped; their images follow as one user message
(preceded by an assistant bridge when required). Tool-call arguments are written as
compact JSON with array-index keys first in numeric order and floats spelled as
ECMAScript prints them.

## Stream and result

Chunks update one shared message; a field of the wrong type reads as absent, a chunk
that is not an object is skipped, and events named `thread.*` are ignored. Text and reasoning each keep one open block; tool
calls are matched by their stream index, then by ID. The first non-empty response ID
and the first returned model name that differs from the requested one are retained.
Usage from the chunk (or, failing that, its first choice) replaces the running usage;
cache writes are removed from reported cache hits and costs follow the model's rates.
Finish reasons map to `stop`, `length` and `toolUse`; any other reason ends in an
error. A stream that ends without a finish reason keeps its initial `stop`.
Late encrypted reasoning details are attached to the matching tool call.

Every update and the final message share the same handle. No lock is held while
readers are woken. On failure the partial content is kept and the last update is an
error (or an abort, when the signal was aborted). A body that reports its own abort
while the signal is unset is a failure (`Request was aborted.`), not a normal end. After
a normal end the shared message decides the result: stop reasons `stop`, `length` and
`toolUse` complete, an aborted stop reason fails with `Request was aborted`, and an
error stop reason fails with its own text, or `Provider returned an error stop reason`
when that is empty.

## Transport

The default transport is `reqwest`, available as `default_fetch()` for plain requests that
need no retry policy; replace it with `StreamOptions::fetch`, a function from
`HttpRequest` to `HttpResponse`. A request is retried when the
connection fails, the setup timeout elapses, or the status is 408, 409, 429 or 5xx;
`x-should-retry: true` or `false` overrides the status. The default is two retries
and a 600,000-millisecond timeout that ends when the response headers arrive, so a
slow body is not cut short. A wait comes from `retry-after-ms` (when nonzero), then
`retry-after` as seconds or an HTTP date (past dates mean no wait); otherwise
`min(0.5 * 2^n, 8)` seconds reduced by up to a quarter at random. Invalid hints are
ignored. The timeout and retry count must be whole numbers of at least zero:
`timeout must be an integer`, `timeout must be a positive integer` and the same two
messages for `maxRetries` reject anything else before a request is sent, including a
retry count beyond what a 64-bit integer holds. The retry count is an exact integer
budget, so even a very large count is used up one retry at a time. Aborting the signal
interrupts a pending request or retry wait, and on browser targets the timers of
requests that finish, fail or are aborted are cleared.

Failures read `Connection error.`, `Request timed out.`, `Request was aborted.`,
`{status} {message}`, `{status} status code (no body)` or `(no status code or body)`.
When a provider reports `error.metadata.raw`, it is appended on a new line.

## Example

```rust
use std::sync::Arc;

use futures_util::stream;
use maestro_models::{
    AssistantMessageEvent, Context, HttpResponse, Model, OpenAICompletionsOptions, StreamOptions,
    stream_openai_completions,
};

let model: Model = serde_json::from_value(serde_json::json!({
    "id": "controlled", "name": "Controlled", "api": "openai-completions",
    "provider": "controlled", "baseUrl": "https://example.invalid/v1",
    "reasoning": false, "input": ["text"], "contextWindow": 1000, "maxTokens": 100,
    "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0}
}))?;
let context: Context = serde_json::from_value(serde_json::json!({
    "messages": [{"role": "user", "content": "hello", "timestamp": 0}]
}))?;
let answer = concat!(
    r#"data: {"id":"r","model":"controlled","choices":[{"index":0,"#,
    r#""delta":{"content":"hi"},"finish_reason":"stop"}]}"#, "\n\n",
    "data: [DONE]\n\n",
);
let options = OpenAICompletionsOptions {
    common: StreamOptions {
        api_key: Some("controlled-key".into()),
        fetch: Some(Arc::new(move |request| {
            assert_eq!(request.url, "https://example.invalid/v1/chat/completions");
            Box::pin(std::future::ready(Ok(HttpResponse {
                status: 200,
                headers: Default::default(),
                body: Box::pin(stream::iter([Ok(answer.as_bytes().to_vec())])),
            })))
        })),
        ..StreamOptions::default()
    },
    ..OpenAICompletionsOptions::default()
};
let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
let text = runtime.block_on(async {
    let stream = stream_openai_completions(model, context, Some(options));
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
