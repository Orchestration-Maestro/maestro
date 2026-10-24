# Interactive commands

The interactive mode reads submitted editor text before it becomes prompt input.
Text it recognizes as a command runs in the mode and answers on the transcript;
every other submission reaches the [typed input](interactive-mode.md#typed-input)
and the editor history as before.

Today the mode handles shell input and the `/export`, `/name`, `/session` and
`/debug` commands. Import, the offline viewer and packaged asset injection are not
delivered yet (see [what is missing](#what-is-missing)).

`maestro_chat::InteractiveMode` exposes the bodies of the session commands as
methods. Each method reads the runtime's current session when it is called, so it
follows a session switch.

Editor submission routes typed commands to them after trimming the text: `/name`
or `/name ` followed by a name, `/session`, `/debug` and `/export`. Other text,
such as `/names` or `/session extra`, is ordinary input. Editor submission empties
the editor before the command runs, so a command does not reach the waiting input
or the editor's history. When a command fails, the failure goes to the terminal
host.

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

## Session name

`handle_name_command(text)` takes the whole command text. It drops a leading
`/name`, then the outer whitespace under the shared whitespace policy. U+FEFF is
trimmed and U+0085 is kept.

- With no name left, it shows `Session name: <name>` in the dim colour, or the
  warning `Usage: /name <name>` when the session has no name.
- Otherwise it records the name through the session and shows
  `Session name set: <name>`. The confirmation repeats the name that was
  selected, even when a session listener changes the stored name or the editor.

Each notice appends an empty row and a padded text row to the transcript and
requests a frame. A stored name that is not text, or a failure to record the
name, is returned as an error, and no notice is added.

## Session information

`handle_session_command()` reads the current session's statistics and name and
appends one padded text block to the transcript:

- `Session Info` in bold, then `Name:` when the session has a name, `File:` with
  the session file or `In-memory` when there is none, and `ID:`. An empty file
  name is shown as given.
- `Messages`, with the user, assistant, tool call, tool result and total counts.
- `Tokens`, with input, output and total tokens, plus cache reads and cache writes
  only when they are above zero. Token totals use the grouping and decimal
  separator of the locale chosen by `LC_ALL`, `LC_MESSAGES` or `LANG`, with at most
  three decimals.
- `Cost`, only when the cost is above zero, with four decimals. A cost that rounds
  to `0.0000` is still shown.

Labels use the dim colour and headings are bold. A malformed stored record is
returned as the statistics owner's error.

## Debug capture

`handle_debug_command()` renders the presentation at the terminal's width, then
replaces the debug log with a capture, creating its directory. The log is
`maestro-debug.log` in the agent directory, `~/.maestro/agent` unless
`MAESTRO_CODING_AGENT_DIR` names another one. It holds:

1. `Debug output at` with the UTC time, the terminal size and the rendered line
   count.
2. Each rendered line as `[index] (w=width) "text"`, with its visible width and
   its JSON-escaped text, styling included.
3. Each session message as one compact JSON line, in history order.

After the write, the transcript shows `✓ Debug log written` and the log path, so
the capture never contains its own notice. A failed write is returned, and no
notice or frame follows.

The toolkit's global debug key (Shift+Ctrl+D) runs the same capture whatever has
focus. The key handler does not keep the mode alive. A failure is handed to the
terminal host, which reports it.

## What is missing

- Import (`/import <path.jsonl>`) is not delivered. It waits for the shared
  dialog host and the session rebuild helpers, and it stays part of the same work.
- The complete offline viewer is separate work; the export tests supply a
  controlled bundle, so they prove the serializer wiring and not a shipped viewer.
- Packaging and asset injection are separate work. The mode reads the bundle the
  configuration's package directory selects, exactly as the other export callers
  do.
