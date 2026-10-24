# Interactive commands

The interactive mode reads submitted editor text before it becomes prompt input.
Text it recognizes as a command runs in the mode and answers on the transcript;
every other submission reaches the [typed input](interactive-mode.md#typed-input)
and the editor history as before.

Today the mode handles shell input and one slash command, `/export`. Import, the
offline viewer and packaged asset injection are not delivered yet (see
[what is missing](#what-is-missing)).

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

The interrupt key cancels a running shell command when no response streams, and a
streaming response keeps precedence over it: with both running, the key restores the
queued text and stops the response, leaving the shell command to finish on its own.

An accepted shell command is recorded in the editor's history. When an agent response is
streaming, the command's row appears in the pending area and moves into the transcript
with the next ordinary submission; otherwise the row appears in the transcript at once.

Extensions can answer a shell command through the `user_bash` event: returning a full
result shows it without running a process and records it once, and returning operations
runs the command through them instead of the local shell.

## `/export`

`/export` with no path writes the whole session as an HTML document. A path
ending in exactly `.jsonl` writes the current branch as JSONL lines instead.

```text
/export
/export transcript.html
/export "notes from friday.jsonl"
```

Both formats reuse the [session export](../export.md) owners: the HTML document
comes from the same serializer and native bundle that
`AgentSession::export_to_html` uses, and the JSONL lines come from
`AgentSession::export_to_jsonl`. The mode adds no serializer, no asset fallback
and no second file writer.

A success shows `Session exported to: <path>` in the transcript's dim status
colour, naming the path the export owner returned. JSONL returns the
authored path resolved against the process directory; HTML returns the authored
path, or `maestro-session-<session file name>.html` when no path was given.

A recoverable failure shows `Failed to export session: <diagnostic>` in the
transcript's error colour, with the diagnostic the export owner produced. A
failure to render that notice goes to the terminal runtime's error route, which
is the route the mode's shortcuts already use.

## How the path is read

The reader recognizes the command token, then reads one path. It is token
reading, not shell parsing:

- The text must be exactly `/export`, or `/export` followed by one space. A
  prefix such as `/exporter` or `/export` followed by a tab is not the command,
  and that text is delivered as prompt input.
- After that single space, leading whitespace is removed under the shared
  [whitespace policy](../terminal/text.md): Unicode white space other than
  U+0085, plus U+FEFF.
- A quoted path returns the text between the first quote and the next matching
  quote, including empty text. An unterminated quote leaves no path, so the
  command exports HTML under the default name.
- A bare path ends at the first whitespace character. Later tokens are ignored.
- Nothing is unescaped, expanded or normalized: `~/notes.jsonl` and `"notes
  from friday.jsonl"` reach the export owner as authored.

The first matching quote wins, so `"/export/one"` is the path `/export/one` and a
second quote character inside the path ends it. Apostrophes are ordinary
characters unless the path starts with one.

## Which format is chosen

Only an exact lower-case `.jsonl` suffix selects JSONL. `.JSONL`,
`archive.jsonl.bak`, `notes.jsonl-summary` and an absent or empty path all select
HTML. The comparison stays case-sensitive on purpose: the reference command
selects JSONL for that suffix alone.

## What is missing

- Import (`/import <path.jsonl>`) is not delivered. It waits for the shared
  dialog host and the session rebuild helpers, and it stays part of the same work.
- The complete offline viewer is separate work; the export tests supply a
  controlled bundle, so they prove the serializer wiring and not a shipped viewer.
- Packaging and asset injection are separate work. The mode reads the bundle the
  configuration's package directory selects, exactly as the other export callers
  do.
