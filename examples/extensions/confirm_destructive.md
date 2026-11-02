# Confirm destructive

Build with `--example confirm_destructive` using the [build command](README.md#building-an-example). Loading is
opt-in and needs a component-enabled host. Interactive confirmation needs that
host's dialog and notification UI; headless hosts continue without a veto.

Starting a new session always asks **Clear session?**, even when history is empty.
Resuming another session asks **Switch session?** only when the current session
contains a user message anywhere in its entries. Other entry kinds do not count.
Declining or dismissing either confirmation reports cancellation and vetoes the
replacement. Approving returns no override.

Before a fork, the selector shows the first eight characters of the entry ID.
Only **Yes, create fork** permits it; **No, stay in current session**, dismissal
and any other answer report `Fork cancelled` and veto it. The host owns session
replacement and forking: this example only returns cancellation decisions and
never deletes messages or changes files itself.

The controlled `confirm_destructive_` runs replay scripted history and UI answers
through both the in-process adapter and the built component. They need no
provider, credentials, filesystem changes or extension installation. A host
rejection propagates unchanged instead of becoming an approval.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- confirm_destructive_
```
