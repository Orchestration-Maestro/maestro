# Status line

`session_start` sets `status-demo` to dim `Ready`. Each `turn_start` increments
this factory's counter and shows an accent `●` followed by dim ` Turn N...`.
`turn_end` shows a success `✓` followed by dim ` Turn N complete`. Each hook
reads the current UI theme; the counter stays in place across session starts
and starts at zero in a new factory instance. A host rejection returns its
message without undoing a counted turn.

Build with `<name>` set to `status_line` in the [build command](README.md#building-an-example). The
controlled runs invoke the built component with supplied theme and UI answers;
they need no provider, credentials or installation. Loading it into an
application is opt-in and needs that application's component runtime. A UI host
must provide footer status and theme operations to show the status.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- status_line_
```
