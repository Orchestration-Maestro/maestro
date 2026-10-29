# Inline bash expansion

Load this example only when you want shell commands embedded in prompts to run.
`The current directory is !{pwd}` replaces the expression with trimmed shell
output before submission. Multiple expressions run in order, in the session's
working directory, with a 30-second timeout per command. Bash receives each
command as one `-c` argument, preserving shell quoting.

Ordinary whole-line `!command` input passes through unchanged. Empty `!{}` and
unclosed expressions also pass through. Expressions stop at the first `}`;
this is not a parser for nested shell braces. Expansion output is not executed
again. Output prefers stdout, then stderr, then an empty string. Nonzero exits
with stderr appear in the notification summary; execution errors replace the
expression with `[error: <message>]`. Attachments stay with the prompt.

When a notification UI is available, one informational summary lists the
commands and their first 50 characters of output, with `...` for longer output.
Without a UI, expansion still works. This hook applies to all input sources,
including extension-injected input.

## Build and load

Build with `--example inline_bash` in the
[component build command](README.md#building-an-example). Load the resulting
component through the [extension guide](../../docs/extensions.md). Loading is
opt-in and requires Bash on the host and a component-enabled Maestro host.
Notifications additionally require a UI. There are no new settings or flags.

Commands run with your permissions and can read, write or delete files. Use
only trusted prompts: command output becomes model input and may expose secrets.
The host's configured model handles the expanded prompt and may incur provider
charges. The controlled tests do not contact a provider or use credentials.

## Controlled run

```sh
CARGO_BUILD_JOBS=3 ~/.local/bin/capped mise exec -- cargo nextest run --locked \
  -p maestro-extensions-wasmtime --test integration -- inline_bash_
```

The tests load the compiled shipped component, execute literal local children
inside disposable directories and record notifications without a terminal.
