# Event bus

`/emit [message]` trims its argument and sends it on the fixed `my:notification`
channel with `"from": "/emit command"`, defaulting to `hello` when nothing is
left. The argument is always the message: the command takes no event name, so
`/emit other:event payload` sends that whole text to the same channel.

A retained listener reports each `my:notification` payload through the current
session context's UI as `Event from {from}: {message}` at information severity.
The example reports one notification at session start, keeps the latest session
context when a session is replaced, and reports nothing before the first session
starts.

Any extension can emit on the same channel with an object holding string
`message` and `from` members; other members are ignored. Loading the component
into an application is opt-in and needs that application's component runtime.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- event_bus_component_uses_registered_callbacks
```
