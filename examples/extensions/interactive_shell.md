# Interactive shell

Build with `--example interactive_shell` using the
[build command](README.md#building-an-example). Load the component explicitly
on a native Unix host with an interactive terminal and `/bin/sh` or a usable
`SHELL` environment variable. Loading is opt-in; the example runs shell code
with your permissions and inherited standard input, output and error.

Type `!vim file.txt`, `!git rebase -i HEAD~3`, or `!htop` to hand the terminal
to the command. Type `!i any-command` to force interactive execution. Only user
`!` commands are intercepted; agent tool calls keep their normal handling.
Commands run in the session's working directory with the host environment.

`INTERACTIVE_COMMANDS` adds comma-separated command prefixes.
`INTERACTIVE_EXCLUDE` removes comma-separated prefixes, ignoring case.
Whitespace around entries is ignored. Exclusion removes a configured prefix,
not an individual invocation; `!i` bypasses detection and exclusion. Matching
accepts a command at the start or after the last pipe, not shell syntax in
general. Quoted commands, aliases and compound shell expressions are not parsed.

The terminal stops before the child starts and restarts with a full redraw
after it exits or launch/wait fails. Numeric child status is returned; signal
termination uses exit code 1. Headless runs return exit code 1 without spawning.
No provider, credentials or live service is required.

## Controlled runs

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- interactive_shell_
```

These tests load the shipped component through the extension runner. Process
and terminal inputs are controlled; no editor, remote shell or database is
started. Use a disposable directory when trying interactive commands yourself.
