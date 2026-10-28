# Pirate

`/pirate` starts disabled. Each invocation reports the state it switched to and
ignores its arguments. While the mode is on, `before_agent_start` returns the
supplied prompt followed by this example's own pirate instructions; while it is
off, the handler returns no override and leaves the event as it arrived. The
state belongs to the factory instance, so two loaded copies toggle apart, and a
host that refuses the notification still keeps the switch it made first. Loading
the component into an application is opt-in and needs that application's
component runtime.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- pirate_
```
