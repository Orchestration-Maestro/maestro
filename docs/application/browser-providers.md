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
diagnostics. The default transport recovers browser exception names from the
retained client error text, preserving the full message; ordinary connection
failures remain named `FetchError`. Classification is not proof that a browser
security policy caused a failure.

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
`DiscoveryOptions`: the shared HTTP operation, complete-message socket factory
and synchronous diagnostic callback. Native callers can supply
[`default_fetch`](../../crates/maestro-models/src/providers/transport/mod.rs) and
[`default_web_socket`](../../crates/maestro-models/src/providers/websocket/native.rs).
Browser callers supply
[`browser_web_socket`](../../crates/maestro-web/src/browser/websocket/browser.rs).
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
- LM Studio authenticates a fresh `/system` WebSocket and calls
  `listDownloadedModels`. It validates the downloaded records before filtering
  embeddings, uses downloaded paths as IDs, and preserves names and both context
  limits. Tool training selects reasoning independently of vision's image input.
  Context limits must be integral numbers, including zero, negative and wide
  values. The socket destination uses `ws`, the parsed hostname and parsed port
  or 1234; invocation URLs still append `/v1` to the authored base.

The supplied key is used only for llama.cpp and vLLM, and only when nonempty.
Successful listings retain order and duplicates. Ollama detail failures are
reported and omitted independently; malformed whole listings fail discovery.
Invalid IDs and nonnumeric or nonfinite selected limits do not become models.
HTTP discovery admits valid zero, negative and fractional limits without a
positive-integer restriction. Every failure exposes its provider and category; transport causes
remain available through `Error::source`.

HTTP responses use the shared
[`decode_utf8`](../../crates/maestro-text/src/decoding.rs); socket JSON does not strip
a leading BOM. Both use
[JSON object admission](../../crates/maestro-json/src/object.rs).
Selected duplicate members keep their last value. HTTP discovery leaves unrelated
fields unread; LM Studio also validates its unused declared metadata.
Diagnostics distinguish tools skips, detail failures and whole-operation HTTP
failures. LM Studio returns failures without duplicate diagnostics, retaining
socket phases, authentication causes and complete remote error objects. Malformed
replies fail immediately. Its owned socket closes on success, failure or future
drop, without waiting for peer acknowledgement.
