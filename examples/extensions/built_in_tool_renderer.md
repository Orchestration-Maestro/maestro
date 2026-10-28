# Built-in tool renderer

Build with `--example built_in_tool_renderer` using the [build command](README.md#building-an-example). Loading is
opt-in and needs a Unix component-enabled host with filesystem access, a working
directory and a shell on `PATH`. The extension replaces the read, bash, edit and
write displays while keeping the original factories' schemas, argument preparation,
execution, cancellation and progress. It adds no permission policy: invoked tools
can still read or overwrite files and run shell commands.

Collapsed results show read line counts or image status, shell exit status, edit
diff statistics and write status. Expanded results show at most 15 read lines,
20 shell lines or 30 diff lines, with truncation notices. Edit keeps its own outer
shell. Long commands keep at most 77 UTF-16 units on whole Unicode scalars before
`...`; commands of at most 80 units remain unchanged. Failed results use the
host's error observation instead of appearing successful. The controlled
`built_in_tool_renderer_` run uses disposable files, fixed shell commands and
supplied render rows. It needs no provider, credentials or extension installation.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- built_in_tool_renderer_
```
