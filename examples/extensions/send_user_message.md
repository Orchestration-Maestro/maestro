# Send user message

`/ask`, `/steer`, `/followup` and `/askwith` send ordinary, steering, follow-up
and two-block user messages. The controlled run invokes the factory on the
controlled host and the built component.

## Controlled runs

```sh
just prepare-tests
cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- user_delivery_
```
