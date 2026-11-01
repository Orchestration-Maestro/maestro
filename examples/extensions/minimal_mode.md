# Minimal mode

Build with `--example minimal_mode` using the [build command](README.md#building-an-example), then
load `target/wasm32-wasip2/release/examples/minimal_mode.wasm` through the
[extension loader](../../docs/extensions/loading.md). Loading is opt-in.

The example replaces the display of `read`, `bash`, `write`, `edit`, `find`,
`grep` and `ls`, not their native execution. Collapsed file and shell results
are hidden; searches and listings show counts. Expanded rows show the first
text block without an additional display limit. Read image attachments retain
the host's image handling; this text renderer does not create image previews.
The existing expand-tools keybinding switches collapsed and expanded output;
this example adds no third mode or keybinding.

Use a Unix native component host with the built-in tool factories enabled and
a terminal UI to see the rows. Shell and search tools keep their normal shell,
`fd` and `rg` prerequisites. Loading does not run tools, but invoking `write`,
`edit` or `bash` can change files or run commands with normal host permissions.
Use a disposable directory for a walkthrough. No provider or credentials are
needed for the controlled run below.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- just prepare-tests
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- minimal_mode_
```
