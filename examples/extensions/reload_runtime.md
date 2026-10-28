# Reload runtime

`/reload-runtime` awaits the reload request its command context supplies and
returns nothing else. The `reload_runtime` tool runs with the ordinary context,
which has no reload method, so it queues `/reload-runtime` as a follow-up user
message and answers that it queued it. The controlled run reloads nothing in a
running application; loading the component is opt-in and needs that
application's component runtime.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- reload_runtime_component_uses_registered_callbacks
```
