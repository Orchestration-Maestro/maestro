# Working indicator

`/working-indicator` reports the current mode. Its `dot`, `pulse`, `none` and
`spinner` arguments choose a static dot, a four-frame pulse at 120 ms, no inline
indicator, or a ten-frame spinner at 80 ms. `reset` restores the host's default
spinner. Arguments ignore surrounding whitespace and case; an unknown argument
reports usage without changing the selection.

Each loaded instance starts with the custom spinner selected. Every
`session_start` applies the selection and sets the `working-indicator` status to
`Indicator: <mode description>`, coloured through the host theme's `dim` role.
The example supplies frames and intervals; the host owns animation and timing.

Build it with `--example working_indicator` using the [index build command](README.md#building-an-example). The
controlled `working_indicator_` runs need no provider, credentials, timer or
installation. Loading the built component is opt-in and needs a component-enabled
host; showing the indicator and status also needs its interactive UI.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- working_indicator_
```
