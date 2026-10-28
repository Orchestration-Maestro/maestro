# Working message test

`session_start` requests the custom working text and a one-frame coloured
indicator through the context it receives, then returns no event result. Every
session-start reason sends the same two requests in that order, and a refusal
the host returns ends the callback with the host's own message.

The controlled run asserts the requests the guest sends. It does not show that
an interactive host keeps the text across agent turns or loader recreation;
that behaviour belongs to the host's own working-loader operations. Installing
the component into an application is opt-in and needs that application's
component runtime.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- working_message_
```
