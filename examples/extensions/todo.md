# Todo

The `todo` tool accepts `list`, `add` with nonempty `text`, `toggle` with an `id`,
and `clear`. Text is kept verbatim, including whitespace. Every result carries
all items and the next identifier in its details, even when an action reports an
error. Clearing removes all items and restarts identifiers at one. These are
session-history snapshots, not files and not the default todo package.

On `session_start` and `session_tree`, the example replays only `todo` tool
results on the host's current branch. The newest snapshot wins; a branch without
one starts empty. Tool rows show themed calls and results, with five items in a
collapsed list and all items when expanded.

`/todos` shows all items and their completion count. Escape or Ctrl+C closes the
view. Without UI it reports `/todos requires interactive mode`. Build with
`--example todo`; `todo/view.rs` is its support module, not a separate target.
The controlled `todo_` runs invoke the registered callbacks with supplied branch,
terminal, helper and theme inputs. They need no provider, credentials or files.
Loading the component is opt-in and needs a component-enabled host; the list view
also needs that host's custom UI, key and terminal-width helpers. Using the tool
through a real model can incur provider charges.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- todo_
```
