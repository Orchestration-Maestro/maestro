# Widget placement

On every `session_start`, `widget_placement` reads the host's current UI
availability and does nothing when there is none. With a UI it requests the
`widget-above` widget holding the line `Above editor widget`, then the
`widget-below` widget holding the line `Below editor widget` with the explicit
below-editor placement. The first request passes no options, so the host applies
its own above-editor default.

Each request acquires the UI context again, so a host that changes its binding
after the first request serves the second on the new one. A rejected request
ends the callback with that message; the example retries nothing and removes no
widget the host already accepted. Loading the component into an interactive
application is opt-in and needs that application's component runtime and UI
host; the controlled run only records the ordered requests.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- widget_placement
```
