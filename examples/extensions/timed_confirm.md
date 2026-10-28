# Timed confirm

`/timed` asks for confirmation with a five-second countdown. `/timed-select`
offers Option A, Option B and Option C with a ten-second countdown. Both use the
host's dialog timeout and report its answer or dismissal. `/timed-signal` instead
uses a caller-owned `AbortController` and the host's five-second native wait. It
reports approval, user cancellation and expiration separately, and drops the
unfinished wait when the dialog answers first. Only the timeout-option dialogs
request the host's live countdown; the signal variant announces its deadline in
a notification.

Build with `--example timed_confirm` using the [build command](README.md#building-an-example). The `timed_`
controlled runs invoke every command on the built component and the in-process
adapter. Separate dialog and clock barriers distinguish approval, cancellation
and expiration without wall-clock waits. No provider, credentials or installation
is needed. Loading the component is opt-in and needs a component-enabled host with
dialog UI and native-wait support.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- timed_
```
