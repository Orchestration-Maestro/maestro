# Titlebar spinner

`agent_start` restores the base title, then shows the ten braille frames in order
at 80 ms intervals. The first frame waits for that interval. Each frame reads the
current session name and working directory again, producing
`<frame> Maestro - <session> - <directory>`, or omitting the session part when the
name is absent or empty. Restarting cancels the old wait and starts at the first
frame; `agent_end` and `session_shutdown` cancel the wait and restore the base
title. Shutdown also removes the example's event-bus listener.

Build with `--example titlebar_spinner` using the [build command](README.md#building-an-example). Loading it is
opt-in and needs a component-enabled host with native waits, event-tail execution
and terminal-title UI. The controlled runs supply wait-completion gates and
session names, so they need no provider, paid service or terminal installation.
The example starts no process and reads no files. A title-write or name-read
failure ends the callback or animation with the host's error; tail errors go to
the host's event-bus reporter.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- titlebar_spinner_
```
