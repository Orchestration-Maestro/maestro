# Bookmark

`/bookmark` labels the newest assistant message and `/unbookmark` removes the
newest label from any entry, so the tree view can show where the important
points of a session are. A blank argument becomes a `bookmark-<epoch
milliseconds>` label. The controlled run replays every recorded case against
the built component with a supplied wall clock, so it needs no provider and no
installation; loading the component into an application is opt-in and needs
that application's component runtime.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- bookmark_
```
