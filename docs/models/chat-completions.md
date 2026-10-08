# Chat completions

`maestro-models` sends chat-completion requests directly and streams the answer as
shared assistant updates. Two entry points start a call:

- `stream_openai_completions(model, context, options)` takes the full
  `OpenAICompletionsOptions`: the common `StreamOptions` plus a typed `ToolChoice`
  and a `reasoning_effort`. It returns the call's `AssistantMessageEventStream`, and
  every failure the call detects, including a missing key, ends that stream with an
  error update instead of failing the call.
- `stream_simple_openai_completions(model, context, options)` takes
  `SimpleStreamOptions`. It resolves a nonempty explicit key or the provider's
  environment key first and returns `No API key for provider: {provider}` at once,
  before any stream exists. It then applies `build_base_options`, clamps the
  requested thinking level to what the model supports (absent or `off` requests no
  reasoning) and forwards the typed `tool_choice`.

On native targets the call must run inside a Tokio runtime that has the time driver
enabled (`enable_time`, or `enable_all`); the default HTTP client also needs the I/O
driver. The request work is owned by the runtime, so dropping the returned stream does
not cancel it and `result()` still resolves. Called outside a runtime, the stream ends
at once with `Streaming requires a running Tokio runtime.` A runtime that lacks a
driver the request needs is not detected: Tokio panics inside the request work, and
the stream then never ends. Browser targets use local futures.

## Request

The endpoint is the model's base URL with `/chat/completions` appended: a trailing
slash on the base is not doubled, and the joined text is normalized as a URL
(`https://host/a/../v1` becomes `https://host/v1`). Cloudflare providers first
substitute `{NAME}` placeholders (uppercase letters, digits and underscores, not starting
with a digit) from the environment; an unset or empty variable fails with
`{NAME} is required for provider {provider} but is not set.` See
`providers::chat::cloudflare` for the four endpoint constants and
`is_cloudflare_provider`.

The key is the explicit `api_key` when it is nonempty, then `get_env_api_key(provider)`,
then `OPENAI_API_KEY`; with none of them the call fails with
`OpenAI API key is required. Set OPENAI_API_KEY environment variable or pass it as an argument.`

Headers are layered, later layers replacing earlier ones case-insensitively, repeated
names inside one layer included: the account defaults `openai-organization` and
`openai-project` from the `OPENAI_ORG_ID` and `OPENAI_PROJECT_ID` environment
variables (surrounding whitespace removed; unset or blank variables send nothing),
the model's headers, the dynamic headers of the `github-copilot` provider
(`providers::chat::github_copilot_headers`), session affinity headers
(`session_id`, `x-client-request-id`, `x-session-affinity`) carrying a nonempty session
identifier when the model's compatibility enables them and caching is on,
then the caller's headers. Names are
sent in lowercase. The generated `authorization: Bearer {key}` sits below all layers, so
any layer replaces it; `cloudflare-ai-gateway` sends the key as `cf-aig-authorization`
above all layers instead, and an `Authorization` header supplied in any layer (any
casing) is then kept as the upstream credential.

Compatibility is detected from the provider and base URL, then overridden by
the model's own compatibility record: store and developer-role support, reasoning
effort, usage in streams, tool-result names, assistant messages after tool
results, reasoning replayed as text, `reasoning_content` on assistant messages, the
token-limit field, the reasoning convention (`openai`, `openrouter`, `deepseek`,
`zai`, `qwen`, `qwen-chat-template`), strict tool fields, tool streaming, cache
markers, session affinity and the one-hour cache lifetime. A zero or NaN token limit is
omitted; a zero temperature is sent. A NaN or infinite temperature and an infinite token
limit are sent as `null`, the way JSON text writes them. `OpenRouter` routing preferences are
forwarded only when the base URL contains `openrouter.ai`, and gateway routing preferences
only when it contains `ai-gateway.vercel.sh` and they set `only` or `order`.

Reasoning follows the model's convention. A model that does not reason sends no
reasoning fields. For one that does, `openai` sends a requested level as
`reasoning_effort` (only where the endpoint accepts that field), `deepseek` as
`reasoning_effort` with `thinking: enabled`, `openrouter` as `reasoning.effort`, `zai` and
`qwen` as `enable_thinking: true`, and `qwen-chat-template` as `enable_thinking: true`
(with `preserve_thinking: true`) in `chat_template_kwargs`. Where a level is sent by name
(`openai`, `deepseek`, `openrouter`), the name is the model's `thinking_level_map` entry,
or the level's own name when the map has none (a `null` entry included). With no level
requested, `zai` and `qwen` send `enable_thinking: false`, `qwen-chat-template` sends it
in `chat_template_kwargs`, `deepseek` sends `thinking: disabled`, `openrouter` sends the
map's `off` name (effort `none` when the map has no entry, nothing when it is `null`) and
`openai` sends the map's `off` name when it has one, again only where the endpoint accepts
`reasoning_effort`.

Cache retention is the explicit option, otherwise `long` when
`MAESTRO_CACHE_RETENTION` is exactly `long`, otherwise `short`. `none` suppresses
prompt-cache and affinity fields. Cache markers, where the compatibility selects the
`anthropic` style (by default the `openrouter` provider with a model whose ID starts with
`anthropic/`), go on the first instruction, the last tool and the last conversation text;
with `long` retention they carry a one-hour lifetime where the compatibility supports it.
`long` sends the `24h` retention and the session key only where the compatibility supports
long caching; direct `OpenAI` URLs send the session key for any retention but `none`.

`on_payload` receives the payload as JSON before it is sent (the temperature and token
limit as floating-point numbers, or `null` when they are not finite) and returns the
payload to send: the one it received, edited or not, or a new one. `on_response` receives
the status and headers of the accepted response before the `start` update; it is not
called for responses that are retried. Both hooks also receive the one model the request
holds, shared and read-only. A hook error ends the stream with that error.

## History

`convert_messages` projects the history for the model, then writes wire messages.
Tool-call IDs from other models' turns that contain `|` are cut at it and every
character outside `[A-Za-z0-9_-]` becomes one `_` per UTF-16 unit, up to 40 units.
Other such IDs are kept, except for the `openai` provider, where they are cut to 40
UTF-16 units at a character boundary. Blank assistant text and reasoning are dropped:
spaces, line breaks, the no-break space and the byte-order mark count as blank, U+0085
and U+200B do not. User text is kept as it is, and kept assistant text is never trimmed.
Instructions use the developer role when the model reasons and the endpoint supports it.
Tool results are grouped; when the model accepts images, their images follow as one user
message (preceded by an assistant bridge when the endpoint requires one). Tool-call
arguments are written as compact JSON with array-index keys first in numeric order and
floats spelled as ECMAScript prints them.

## Stream and result

The body is read as server-sent events the way the `OpenAI` client library reads them; a
recorded corpus of that library's results is part of the tests, and the reader belongs to
`maestro-models`, which depends on no event-stream library. Lines end at `\r\n`, `\r`
or `\n`, wherever the chunks are cut. Each line is decoded as text on its own: bytes that
are not valid UTF-8 become U+FFFD, a character cut by the end of a chunk is completed by
the next one before its line is read, and one leading byte-order mark is dropped from the
line. A second mark stays in the line, so a line that starts with two reads the second as
part of its field name. Bytes that are not text therefore never fail the call by
themselves: they become replacement characters in the line they occur in, and a line made
only of them is an unknown field.

A blank line delivers the event collected so far: its `data` fields joined by `\n` and its
`event` name, if any. One space after the colon of a field is dropped. Comment lines and
fields other than `event` and `data` are ignored. A final line without a line ending is
still read, but an event whose blank line never arrives is not delivered.

Chunks update one shared message; a field of the wrong type reads as absent, a chunk
that is not an object is skipped, and events named `thread.*` are ignored. Text and
reasoning each keep one open block; tool calls are matched by their stream index, then
by ID. Stream indices compare as numbers, so `0`, `0.0`, `0e0` and `-0` are one index,
and a fragment with a known index continues its call whatever ID it carries. The first
non-empty response ID and the first returned model name that differs from the requested
one are retained. Usage from the chunk (or, failing that, its first choice when the chunk
has none) replaces the running usage; when cache writes are reported they are removed from
reported cache hits, and costs follow the model's rates. `stop` and `end` finish with `stop`,
`length` with `length`, and `function_call` and `tool_calls` with `toolUse`; any other
reason sets an error stop reason with the text `Provider finish_reason: {reason}`. A later
finish update replaces an earlier one, so an unknown reason fails the call only when it is
the last one received; an empty reason is ignored. A stream that ends without a finish
reason keeps its initial `stop`. An encrypted reasoning detail (`reasoning.encrypted`,
with a nonempty ID and a `data` value that is not null, false, zero or empty text) is
attached to the tool call with that ID when that call is already open, and dropped
otherwise.

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
`HttpRequest` to `HttpResponse`. A request is retried when the connection fails, the setup
timeout elapses, or the response status is 408, 409, 429 or at least 500;
`x-should-retry: true` or `false` overrides the status. A 2xx response is accepted as it
is: neither `x-should-retry` nor the retry hints below are read for it. The default is
two retries and a 600,000-millisecond timeout that ends when the response headers arrive,
so a slow body is not cut short. Before a retry of a response, the wait comes from
`retry-after-ms` when it is nonzero, else from `retry-after` as seconds or an HTTP date (a
past date means no wait), else from a zero `retry-after-ms`; with no usable hint, and after
a connection failure or a timeout, it is `min(0.5 * 2^n, 8)` seconds reduced by up to a
quarter at random. Invalid hints are ignored. The timeout and retry count must be
whole numbers of at least zero: `timeout must be an integer`, `timeout must be a positive
integer` and the same two messages for `maxRetries` reject anything else before a request
is sent, including a retry count beyond what a 64-bit integer holds. The retry count is an
exact integer budget, so even a very large count is used up one retry at a time. Aborting
the signal interrupts a pending request, the read of an error body or a retry wait, and on
browser targets the timers of requests that finish, fail or are aborted are cleared.

Failures read `Connection error.`, `Request timed out.`, `Request was aborted.`,
`{status} {message}`, `{status} status code (no body)` or, for a status of 0 with an empty
body, `(no status code or body)`. When a provider reports a nonempty `error.metadata.raw`,
it is appended on a new line. An error body
is decoded as one text: a leading byte-order mark is dropped, so it does not hide a JSON
error object, and invalid bytes appear as U+FFFD.

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
