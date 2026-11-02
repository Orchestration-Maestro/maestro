# Mac system theme

Build with `--example mac_system_theme` using the [build command](README.md#building-an-example).
Loading it is opt-in and needs a native component-enabled macOS host with
`osascript` on `PATH`, permission to query System Events, theme operations and
native waits. On each session start it selects `dark` when the appearance command
succeeds with trimmed output `true`, otherwise `light`. Every two seconds it
queries again and requests a theme only when the appearance changes. A failed or
terminated command falls back to light, including after startup. Theme selection
results belong to the host; the example does not install themes or change system
appearance.

Restarting a session cancels the previous poller before selecting the initial
theme again. Shutdown drops the pending wait and removes the listener. The
controlled `mac_system_theme_` runs supply command results and wait completions,
including a built component, so they need no macOS desktop, provider or credentials.
`support/tail.rs` also owns cancellable event-tail work for the title spinner,
Snake and question extractor; each example keeps its own interval and policy.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- mac_system_theme_
```
