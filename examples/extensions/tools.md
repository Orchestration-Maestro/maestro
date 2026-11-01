# Tools

`/tools` opens **Tool Configuration**, with one enabled/disabled setting for each
currently available tool, including extension tools. Each change immediately
updates the active tools and appends a `tools-config` session entry containing
`enabledTools`. Escape closes the view without undoing those changes.

On session start and tree navigation, the example reads only the current branch.
It restores the newest saved selection, discarding tools no longer available;
an empty saved selection disables every tool. Without saved configuration it
keeps the host's active selection. Opening the view refreshes the inventory, so
new tools appear disabled until selected.

The controlled run uses supplied tool inventories, branch entries and terminal
input. The native session run also checks active tools, persisted entries, tree
navigation and reload. Neither run calls a provider or changes project files.
Loading the built component
is opt-in and needs a component-enabled interactive host with custom UI and
settings widgets; loading it permits changes to that session's active tools and
configuration entries.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- tools_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- tools_component_changes_native_session_and_restores_branch
```
