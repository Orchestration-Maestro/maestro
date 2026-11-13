# Chat completions

`maestro-models` sends chat-completion requests directly and streams the answer as
shared assistant updates. Two entry points start a call:

- `stream_openai_completions(model, context, options)` takes the full
  `OpenAICompletionsOptions`: the common `StreamOptions` plus a typed `ToolChoice`
  and a `reasoning_effort`. It returns the call's `AssistantMessageEventStream`.
  Every failure the call detects, a missing key included, ends that stream with an
  error update; the call itself does not fail.
- `stream_simple_openai_completions(model, context, options)` takes
  `SimpleStreamOptions`. It resolves a nonempty explicit key or the provider's
  environment key first. With neither, it returns `No API key for provider: {provider}`
  at once, before any stream exists; the root export of the same name settles that
  error as an error stream (see the [linkage guide](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/models/module-linkage.md)).
  It then applies `build_base_options`, clamps the requested thinking level to what
  the model supports (absent or `off` requests no reasoning) and forwards the typed
  `tool_choice`.

The direct `_for` counterparts take a fourth `ProviderSpawner<E>` argument and
accept `OpenAICompletionsOptions<E>` or `SimpleStreamOptions<E>`. With
`E = Local`, hooks and their futures can keep caller-local state, and the
admission function you supply owns the producer. The root simple `_for` wrapper
still settles setup errors as a stream. The default entry points keep `Threaded`
options and the three-argument syntax.

On native targets the default call must run inside a Tokio runtime with the time
driver enabled (`enable_time` or `enable_all`); the default HTTP client also needs
the I/O driver. The runtime owns the request work, so dropping the returned stream
does not cancel it and `result()` still resolves. Called outside a runtime, the
stream ends at once with `Streaming requires a running Tokio runtime.` A runtime
that lacks a driver the request needs goes undetected: Tokio panics inside the
request work and the stream never ends. Browser targets use local futures.

## Request

The endpoint is the model's base URL with `/chat/completions` appended. A trailing
slash on the base is not doubled, and the joined text is normalized as a URL
(`https://host/a/../v1` becomes `https://host/v1`). Cloudflare providers first
substitute `{NAME}` placeholders (uppercase letters, digits and underscores, not
starting with a digit) from the environment. An unset or empty variable fails with
`{NAME} is required for provider {provider} but is not set.` See
`providers::chat::cloudflare` for the four endpoint constants and
`is_cloudflare_provider`.

The key is the explicit `api_key` when it is nonempty, then
`get_env_api_key(provider)`, then `OPENAI_API_KEY`. With none of them the call
fails with
`OpenAI API key is required. Set OPENAI_API_KEY environment variable or pass it as an argument.`

Headers are layered, and a later layer replaces an earlier one case-insensitively;
repeated names inside one layer count too. From first to last:

1. The account defaults `openai-organization` and `openai-project`, from the
   `OPENAI_ORG_ID` and `OPENAI_PROJECT_ID` environment variables. Surrounding
   whitespace is removed as ECMAScript trims it: the no-break space and the
   byte-order mark count, U+0085 does not. An unset variable sends nothing, and a
   variable that is set but blank sends an empty header.
2. The model's headers.
3. The dynamic headers of the `github-copilot` provider
   (`providers::chat::github_copilot_headers`).
4. The session affinity headers (`session_id`, `x-client-request-id`,
   `x-session-affinity`). They carry a nonempty session identifier when the
   model's compatibility enables them and caching is on.
5. The caller's headers.

Names are sent in lowercase. The shared HTTP layer keeps each normalized name's
first insertion position and its last value; validation stays at the HTTP send
seam.

A header value is text whose characters each name one byte. Tab, line feed,
carriage return and space are removed from both ends of every value in the final
request, and a value that becomes empty is sent empty. No other character counts
as whitespace there.

A header that breaks the Fetch Standard's header rules ends the stream with an
error update before any attempt, retry or call to a replacement client. Three
cases break the rules: an invalid name, a character above U+00FF (U+FEFF included,
since it is not HTTP whitespace), and a value with an interior line break or NUL.
Every other value, control characters such as U+0001 included, reaches a
replacement client unchanged. The default client sends each character as the
single byte it names, so `é` goes out as the byte E9. A value it cannot carry (a
control character other than tab, or DEL) fails that attempt as a connection
failure, which is retried like any other and ends as `Connection error.` The
browser bridge of the default client reads header values as ASCII, so a value with
a non-ASCII character fails there; this is a known limitation of the browser
client.

The generated `authorization: Bearer {key}` sits below all layers, so any layer
replaces it. `cloudflare-ai-gateway` instead sends the key as
`cf-aig-authorization` above all layers, and an `Authorization` header supplied in
any layer (in any casing) is then kept as the upstream credential.

Compatibility is detected from the provider and base URL, then overridden by the
model's own compatibility record. The record covers:

- store and developer-role support, reasoning effort, usage in streams,
  tool-result names, assistant messages after tool results, reasoning replayed as
  text and `reasoning_content` on assistant messages;
- the token-limit field;
- the reasoning convention (`openai`, `openrouter`, `deepseek`, `zai`, `qwen`,
  `qwen-chat-template`);
- strict tool fields, tool streaming, cache markers, session affinity and the
  one-hour cache lifetime.

A zero or NaN token limit is omitted, and a zero temperature is sent. A NaN or
infinite temperature and an infinite token limit are sent as `null`, the way JSON
text writes them. `OpenRouter` routing preferences are forwarded without field
selection whenever the supplied value is truthy, whatever the base URL, so a proxy
in front of the gateway still receives them. Gateway routing selects only a truthy
`only` and `order`, in that order, whatever the base URL; an empty array is truthy.

Reasoning follows the model's convention. A model that does not reason sends no
reasoning fields. For a model that does, a requested level goes out as follows:

- `openai`: `reasoning_effort`, only where the endpoint accepts that field.
- `deepseek`: `reasoning_effort` with `thinking: enabled`.
- `openrouter`: `reasoning.effort`.
- `zai`: `thinking: enabled`.
- `qwen`: `enable_thinking: true`.
- `qwen-chat-template`: `enable_thinking: true` (with `preserve_thinking: true`) in
  `chat_template_kwargs`.

Where a level is sent by name (`openai`, `deepseek`, `openrouter`), the name is the
model's `thinking_level_map` entry, or the level's own name when the map has none (a
`null` entry included). With no level requested:

- `zai` sends `thinking: disabled`.
- `qwen` sends `enable_thinking: false`.
- `qwen-chat-template` sends `enable_thinking: false` in `chat_template_kwargs`.
- `deepseek` sends `thinking: disabled`.
- `openrouter` sends the map's `off` name: effort `none` when the map has no entry,
  nothing when the entry is `null`.
- `openai` sends the map's `off` name when it has one, again only where the endpoint
  accepts `reasoning_effort`.

Cache retention follows the shared
[options policy](../model-options.md#cache-retention). `none` suppresses prompt-cache
and affinity fields. Where the compatibility selects the `anthropic` cache-marker
style (by default the `openrouter` provider with a model whose ID starts with
`anthropic/` or `~anthropic/`), markers go on the first instruction, the last tool
and the last conversation text. With `long` retention they carry a one-hour lifetime
where the compatibility supports it. `long` sends the `24h` retention and the session
key only where the compatibility supports long caching. Direct `OpenAI` URLs send
the session key for any retention but `none`.

`on_payload` receives JSON in `PayloadHookInput::payload` (the temperature and token
limit as floating-point numbers, or `null` when they are not finite) and returns the
selected record. See [hook records](../records.md) for the shared carrier.
`on_response` receives the status and headers of the accepted response before the
`start` update; retried responses do not reach it. Both hooks also receive the one
model the request holds, shared and read-only. A hook error ends the stream with that
error.

## History

`convert_messages` projects the history for the model, then writes the wire messages.

- **Tool-call IDs.** An ID from another model's turn that contains `|` is cut at it,
  and every character outside `[A-Za-z0-9_-]` becomes one `_` per UTF-16 unit, up to
  40 units. Other such IDs are kept, except for the `openai` provider, where they are
  cut to 40 UTF-16 units at a character boundary.
- **Blank text.** Blank assistant text and reasoning are dropped. Spaces, line breaks,
  the no-break space and the byte-order mark count as blank; U+0085 and U+200B do not.
  User text is kept as it is, and kept assistant text is never trimmed.
- **Instructions.** They use the developer role when the model reasons and the endpoint
  supports it. An `OpenRouter` endpoint supports it only for model IDs starting with
  `anthropic/` or `openai/`.
- **Reasoning.** It replays in the field its block's signature names. The `opencode-go`
  provider streams its reasoning as `reasoning` and takes it back as
  `reasoning_content`, so its block carries and replays that name.
- **Tool results.** They are grouped. When the model accepts images, the images follow
  as one user message, preceded by an assistant bridge when the endpoint requires one.
- **Tool-call arguments.** They are written as compact JSON with array-index keys first
  in numeric order and numbers spelled as ECMAScript prints them. Every number is the
  double it names, so an integer beyond 2^53 is rounded.
- **Thought signatures.** A stored signature is replayed when it parses to a value that
  is not null, false, zero or empty text. A number beyond the range of a double is such
  a value, and is written as `null`.

## Stream and result

The body uses the shared server-sent event reader in `maestro-http`; no event-stream
library is involved. The recorded event corpus is part of the tests.

Lines end at `\r\n`, `\r` or `\n`, wherever the chunks are cut. A carriage return ends
its line immediately; a line feed that follows it is skipped, across chunk boundaries
too. Each line is decoded as text on its own:

- Bytes that are not valid UTF-8 become U+FFFD.
- A character cut by the end of a chunk is completed by the next chunk before its line
  is read.
- One leading byte-order mark is dropped from the line. A second mark stays, so a line
  that starts with two reads the second as part of its field name.

Bytes that are not text therefore never fail the call by themselves. They become
replacement characters in the line where they occur, and a line made only of them is an
unknown field.

A blank line delivers the event collected so far: its `data` fields joined by `\n` and
its `event` name, if any. One space after the colon of a field is dropped. Comment
lines and fields other than `event` and `data` are ignored. A final line without a line
ending is still read, but an event whose blank line never arrives is not delivered.

Chunks update one shared message. A field of the wrong type reads as absent, a chunk
that is not an object is skipped, and events named `thread.*` are ignored. A member name
that repeats keeps its last value. Every number is read as the double it rounds to: one
beyond the range of a double becomes an infinity, one too close to zero for a double
becomes a zero, and a number that no field reads cannot fail an event.

- **Blocks.** Text and reasoning each keep one open block. Tool calls are matched by
  their stream index, then by ID. Stream indices compare as numbers, so `0`, `0.0`,
  `0e0` and `-0` are one index, and a fragment with a known index continues its call
  whatever ID it carries.
- **Identity.** The first non-empty response ID is retained, and so is the first
  returned model name that differs from the requested one.
- **Usage.** Usage from the chunk (or, failing that, from its first choice when the chunk
  has none) replaces the running usage. When cache writes are reported, they are
  removed from the reported cache hits. Costs follow the model's rates. A count beyond
  the range of a double stays infinite, and arithmetic on it follows the double rules
  (infinities that cancel give not-a-number).
- **Finish reasons.** `stop` and `end` finish with `stop`, `length` with `length`, and
  `function_call` and `tool_calls` with `toolUse`. Any other reason sets an error stop
  reason with the text `Provider finish_reason: {reason}`. A later finish update
  replaces an earlier one, so an unknown reason fails the call only when it is the last
  one received. An empty reason is ignored. A stream that ends without a finish reason
  keeps its initial `stop`.
- **Encrypted reasoning.** A detail (`reasoning.encrypted`, with a nonempty ID and a
  `data` value that is not null, false, zero or empty text) is attached to the tool
  call with that ID when that call is already open, and dropped otherwise. The attached
  signature is the detail written as compact JSON the way tool-call arguments are, so
  an infinite number in it appears as `null`.

Every update and the final message share the same handle, and no lock is held while
readers are woken. On failure the partial content is kept and the last update is an
error (or an abort, when the signal was aborted). A body that reports its own abort
while the signal is unset is a failure (`Request was aborted.`), not a normal end.
After a normal end, the shared message decides the result: the stop reasons `stop`,
`length` and `toolUse` complete, an aborted stop reason fails with
`Request was aborted`, and an error stop reason fails with its own text, or
`The provider ended the turn with an error stop reason` when that text is empty.

## Transport

The default transport is `reqwest`, available as `default_fetch()` for plain requests
that need no retry policy. Replace it with `StreamOptions::fetch`, a function from
`HttpRequest` to `HttpResponse`.

A request is retried when the connection fails, the setup timeout elapses, or the
response status is 408, 409, 429 or at least 500; `x-should-retry: true` or `false`
overrides the status. A 2xx response is accepted as it is: neither `x-should-retry` nor
the retry hints below are read for it. The default is two retries and a
600,000-millisecond setup timeout that ends when the response headers arrive. The
shared native client has a separate idle limit, which covers body chunks too.

Before a retry of a response, the wait comes from `retry-after-ms` when it is nonzero,
else from `retry-after` as seconds or an HTTP date (a past date means no wait), else
from a zero `retry-after-ms`. With no usable hint, and after a connection failure or a
timeout, the wait is `min(0.5 * 2^n, 8)` seconds reduced by up to a quarter at random.
Invalid hints are ignored.

The timeout and the retry count must be whole numbers of at least zero. Anything else is
rejected before a request is sent, with `timeout must be an integer` or
`timeout must be a positive integer`, and the same two messages for `maxRetries`; a
retry count beyond what a 64-bit integer holds is rejected too. The retry count is an
exact integer budget, so even a very large count is used up one retry at a time.
Aborting the signal interrupts a pending request, the read of an error body and a retry
wait. On browser targets, the timers of requests that finish, fail or are aborted are
cleared.

The shared native client starts with a 300,000 ms idle limit for response headers and
for the gap between body chunks. `configure_http_dispatcher(timeout_ms)` replaces the
client for later requests. Fractional milliseconds are floored, and `0` disables both
limits. Header waiting includes connecting and sending. This setting does not change
private clients or browser transport, and disabling it leaves the provider's setup
timeout in place.

HTTP responses carry `status_text`; see
[`default_fetch`](https://docs.rs/maestro-models/latest/maestro_models/fn.default_fetch.html)
for the default client's platform policy.

Response headers reach `on_response` and the retry rules as one text per name. The
default client reads each native byte of a value as the character of that number (the
byte E9 is `é`), joins repeated values with `, ` (`cookie` values with `; `), keeps
empty values and keeps only the last `set-cookie`. A value carries no whitespace around
it, as RFC 9110 section 5.5 requires of a field value. In a browser the platform has
already decoded the values, and they carry through as UTF-8 text.

Failures read `Connection error.`, `Request timed out.`, `Request was aborted.`,
`{status} {message}`, `{status} status code (no body)` or, for a status of 0 with an
empty body, `(no status code or body)`. A provider `error` value that is not null,
false, zero or empty text counts as an error. Its message is described the way the
client library describes it, with numbers beyond the range of a double written as
`null`. When a provider reports a nonempty `error.metadata.raw`, it is appended on a new
line. An error body is decoded as one text: a leading byte-order mark is dropped, so it
does not hide a JSON error object, and invalid bytes appear as U+FFFD.

## Whole-value conversion

JSON text may nest to any depth. Data that no field reads passes untouched, so an event
or an error body with a deeply nested unread member is read as usual, and a deeply
nested value in a field of the wrong type reads as absent.

Three kinds of value are converted in full: tool arguments, a thought signature, and the
`error` value or `message` that a failure text writes out. Each may hold at most 127
nested arrays or objects. A deeper one is malformed:

- a signature holding it is dropped,
- a failure text writes it as empty text,
- streamed tool arguments fall back to `{}`.

The client library has no such bound; the owner approved the difference, which keeps the
conversion within a fixed recursion depth.

Tool-argument numbers round to binary64 doubles. Valid numeric overflow becomes null,
and negative zero keeps its sign in values (compact JSON writes it as `0`). When strict
argument parsing hits a native magnitude or lone-surrogate error, it retries a raw
projection. The retry accepts overwritten lone-surrogate members only when the surviving
value is representable; if the projection fails, the native error stands.

Full-value conversion keeps every surviving object member or fails. A surviving member
name that holds an unpaired UTF-16 escape cannot fit in a Rust string and fails at the
native decoding boundary. Strict argument parsing keeps its native error. Signature
attachment and replay omit the whole unrepresentable signature, and failure rendering
writes the unrepresentable detail as empty text. Named-field selection skips unread
names instead; see [catalog feed acquisition](generation.md).

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
                status_text: String::new(),
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
