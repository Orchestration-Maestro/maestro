# Send user message

`/ask`, `/steer`, `/followup` and `/askwith` send ordinary, steering, follow-up
and two-block user messages. The controlled run invokes the factory on the
controlled host and the built component.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- user_delivery_
```
