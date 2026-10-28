# Summarize

`/summarize` reads the current branch's user and assistant text, followed by each
assistant message's tool calls. It ignores other entries, images, thinking and
tool results. Empty conversations report `No conversation text found` when a UI
is available. Otherwise it requests a concise, structured summary from
`openai/gpt-5.2` with `reasoningEffort: high`, using the catalog's key and headers.
The model comes from the configured model registry rather than a built-in table, so
the request endpoint matches the authentication that registry supplies.
Missing model or authentication details produce warnings only when a UI is
available. Completion errors return to the host unchanged.

With UI, the response's text blocks appear in a **Conversation Summary** Markdown
view. Enter or Escape closes it; other keys leave it open. The command neither
changes history nor sends the summary back to the conversation.

Build with `--example summarize` using the [build command](README.md#building-an-example). Loading the component is opt-in and requires a
component-enabled host, access to `openai/gpt-5.2` and its credentials; a real run
can incur provider charges. The custom view also requires that host's interactive
UI and widgets. The controlled runs below use a native scripted provider, in-memory
history and supplied terminal inputs, with no network calls or real credentials.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- summarize_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- summarize_
```
