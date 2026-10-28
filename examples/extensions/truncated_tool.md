# Truncated tool

Build with `--example truncated_tool`, then load the component into a Unix
component-enabled host with `rg` on its `PATH`. The opt-in `rg` tool searches the
supplied directory (or the session's current directory), with an optional file
glob. Patterns, paths and globs are literal process arguments, not shell text.

Results retain the first 2,000 lines or 50 KB, whichever limit is reached first.
When truncated, the tool awaits a queued write of the complete output to
`maestro-rg-*/output.txt` in the host's temporary directory before returning its
path and omitted line/byte counts. Expanded results show the first 20 lines and
the full-output path. Each search starts `rg` once through the host's native
process interface. A process that cannot start is a search error; only exit
status 1 with no output means no matches.

Search output can contain private file contents. Temporary artifacts remain on
disk so the read tool can open them; remove their directories when no longer
needed. Loading and searching are opt-in; no provider or paid service is needed
for the controlled runs below. Those runs use disposable files and a controlled
search process through the application's native process owner. They need
`/usr/bin/python3`, not ripgrep.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasm --lib -- truncated_tool_
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro --test integration -- truncated_tool_component_
```
