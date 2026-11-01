# Custom message provider

Build and load `custom_message_provider` to register an opt-in provider with two
reasoning models. Building needs the pinned Rust toolchain and `wasm32-wasip2`
target. Loading needs a component-enabled native host with HTTP access:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo build --locked \
  -p maestro-extensions-wasm --example custom_message_provider \
  --target wasm32-wasip2 --release
maestro --extension target/wasm32-wasip2/release/examples/custom_message_provider.wasm
```

Set `CUSTOM_ANTHROPIC_API_KEY` before starting the host, or run
`/login custom-anthropic` after loading. Select
`custom-anthropic/claude-sonnet-4-5` with `/model`. Real sign-ins and requests
reach the provider and can incur charges.

## Provider reference

The example registers `custom-anthropic` with the `custom-anthropic-api` protocol
at `https://api.anthropic.com`. Its two models, `claude-opus-4-5` and
`claude-sonnet-4-5`, accept text and images. Each has a 200,000-token context
window and 64,000 output tokens.

Subscription sign-in shows an authorization URL. Open it, sign in, then paste
the returned `code#state`. A state that does not match the issued proof key is
rejected. Proof keys come from the host-backed `generate_pkce` helper. Saved
tokens count as expired five minutes early and refresh through the same token
endpoint.

Each request is one HTTP attempt with the conversation, tool schemas and the
requested thinking budget. Without a limit, it asks for a third of the model's
output tokens. The response streams text, thinking and tool calls, followed by
usage and a cost estimate from the model's prices. A missing key fails before
any HTTP request; cancellation settles the message as aborted. A non-success
status ends the message with `HTTP <status>` and the response body, or the
body-read error. The session's automatic retry handles rate limits and server
errors; the example does not retry on its own.

## Controlled examples

These runs use scripted login input and HTTP responses, including a loopback
endpoint for the compiled component. They need no provider credentials or paid
service:

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- custom_message_provider_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped cargo nextest run --locked \
  -p maestro --test integration -- custom_message_provider_
```
