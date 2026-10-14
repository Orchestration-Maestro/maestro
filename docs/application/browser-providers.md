# Browser provider access

The application owns provider access policy in
[`browser::utils`](../../crates/maestro-app/src/browser/utils/mod.rs).
The browser facade exports the four proxy operations; discovery remains an
application operation for callers that manage provider settings.

## CORS proxy

`should_use_proxy_for_provider` selects the proxy for zai and openai-codex,
and for anthropic credentials beginning with `sk-ant-oat` or `{`.
Provider names are case-insensitive, without trimming; other providers connect
directly. `is_cors_error` classifies likely CORS failures from transport or SDK
diagnostics. It is not proof that a browser security policy caused a failure.

`apply_proxy_if_needed` changes only the model base URL when proxying is needed
and both URLs are nonempty. It appends `/?url=` and a URI-component-encoded
base URL literally, without normalizing authored URLs.

`create_stream_fn` reads proxy settings on every invocation, including calls
without a key. A settings failure prevents provider dispatch. A nonempty key
and proxy URL allow rewriting; the adapter forwards the context and complete
simple options to the registered model provider. Separate open options are not
used by this simple-stream adapter.

## Local model discovery

Call `discover_models` or a specialized discovery function with
`DiscoveryOptions`: the shared HTTP operation and a synchronous diagnostic
callback. Production callers can supply
[`default_fetch`](../../crates/maestro-models/src/providers/http/mod.rs).
These operations add no retries, timers or caches.

- Ollama lists tags and concurrently requests details. Only tool-capable models
  survive. Thinking support sets reasoning; models accept text, with zero prices.
  Architecture-selected context defaults to 8192 before decimal-prefix parsing;
  output is ten times context. Request hosts use server-host normalization,
  while returned model URLs append `/v1` to the original authored base.
- llama.cpp reads `<base>/v1/models`, using context and output defaults of
  8192 and 4096 for falsy selected fields.
- vLLM uses the same listing endpoint, selecting `max_model_len` with an 8192
  fallback and capping output at 4096.

The supplied key is used only for llama.cpp and vLLM, and only when nonempty.
Successful listings retain order and duplicates. Ollama detail failures are
reported and omitted independently; malformed whole listings fail discovery.
Invalid IDs and nonnumeric or nonfinite selected limits do not become models.
Valid zero, negative and fractional results are not restricted to positive
integers. Every failure exposes its provider and category; transport causes
remain available through `Error::source`.

Responses use the shared
[`decode_utf8`](../../crates/maestro-text/src/utf8.rs) and
[JSON object admission](../../crates/maestro-json/src/object.rs).
Selected duplicate members keep their last value; unrelated fields stay unread.
Diagnostics distinguish tools skips, detail failures and whole-operation failures.

LM Studio discovery is tracked separately in
[#909](https://github.com/Orchestration-Maestro/maestro/issues/909).
