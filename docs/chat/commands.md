# Editor commands

The interactive mode reads its editor as commands when the submission starts with a
command prefix, and as ordinary input otherwise. This page covers the surface delivered so
far.

## Shell input

Text starting with `!` runs a shell command instead of sending a prompt.

- `!command` runs `command` and keeps its output in the model's context.
- `!!command` runs the same way and records the output as excluded from the context.

Boundaries around the prefix and the command are whitespace: `! ls ` runs `ls`. The
shapes the editor accepts as whitespace are the ones the rest of the mode uses, so a
non-breaking space or a byte-order mark next to the prefix counts as a boundary, while
the next-line and zero-width-space characters do not.

A submission that holds only the prefix, `!` or `!!`, has no command to run and is
delivered as ordinary input instead.

While a shell command from the same session is still running, a new submission does not
start a second one. The editor shows
`A bash command is already running. Press Esc to cancel it first.` as a warning, keeps the
typed text in the editor so nothing is lost, and records no history entry.

An accepted shell command is recorded in the editor's history. When an agent response is
streaming, the command's row appears in the pending area and moves into the transcript
with the next ordinary submission; otherwise the row appears in the transcript at once.

Extensions can answer a shell command through the `user_bash` event: returning a full
result shows it without running a process and records it once, and returning operations
runs the command through them instead of the local shell.
