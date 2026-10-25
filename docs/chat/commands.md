# Interactive commands

The interactive mode reads a few commands from the editor before ordinary
input. Submitted text is trimmed with the shared whitespace rule first: BOM and
no-break space count as whitespace, while U+0085 and zero-width space do not. The
editor empties itself before calling back, so a command needs no clearing step; a
slash command never answers the waiting input and never enters the editor history. Any
other text, including `/Model`, `/models`, `/model<Tab>term`,
`/scoped-models extra` and `/logout extra`, is ordinary input.

The mode handles shell input and the slash commands `/export`, `/changelog`,
`/model`, `/scoped-models`, `/logout`, `/name`, `/session` and `/debug`. Import, the
offline viewer and packaged asset injection are not delivered yet (see
[what is missing](#what-is-missing)).

`maestro_chat::InteractiveMode` exposes the bodies of the session commands as
methods. Each method reads the runtime's current session when it is called, so it
follows a session switch.

Editor submission routes `/name` or `/name ` followed by a name, `/session` and
`/debug` to those methods. Other text, such as `/names` or `/session extra`, is
ordinary input. When a command fails, the failure goes to the terminal host.

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
The row keeps its place above the queued messages while a refresh rebuilds them.

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

## `/changelog`

`/changelog` prints the whole changelog, not only the sections added since the
last run. It reads the changelog beside the running package and appends a
bordered "What's New" card holding every section, in reverse file order.

```text
/changelog
```

The card reuses the [changelog utility](startup.md) for the file read and the
Markdown renderer for the body, so the mode adds no parser and no renderer of its
own. A package whose changelog holds no section shows
`No changelog entries found.` inside the same card. The submission must be
exactly `/changelog`; `/changelogs`, `/changelog extra` and `/CHANGELOG` are
prompt text.

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

## Selecting a model

`/model` opens the model panel in place of the editor. `/model <term>` trims the
term and looks for a unique exact reference among the candidates: the session's
scoped models in their order, repeats included, or, without a scope, the
refreshed catalog's available models. A reference is `provider/id`, compared
case-insensitively with each side trimmed, or a bare id. When exactly one model
matches, it is applied at once; otherwise the panel opens with the term as its
search. Fuzzy matching is left to the panel's search.

The model-select key (`app.model.select`, Ctrl+L by default, configurable in
`keybindings.json`) also opens the panel. The panel saves the chosen model as the
default before the mode applies it. Escape puts the editor back and redraws.

Applying a model selects it on the session, refreshes the footer and the editor
border, puts the editor back when the panel was open and shows
`Model: <id>`. The id is read from the chosen model after the session's model
handlers ran, so a handler that renames it is reflected. When the session
refuses the model, for example because it has no credentials, the editor comes
back and the refusal is shown as `Error: <message>`; a change the session already
made is kept.

The footer shows the provider before the model name when the candidates span
more than one provider. The mode counts them at startup and after every scope
change.

## Editing the model scope

`/scoped-models` refreshes the catalog and opens the scope panel over every
available model, or shows `No models available` when there are none. A refresh
or availability failure is reported as an error instead. The panel starts from
the session's scope, in its order, when there is one; otherwise from the
`enabledModels` setting, resolved against the catalog. A missing or empty setting
enables every model. A setting that is not a list is treated as unset, and a list
with a member that is not text is rejected before any pattern is resolved.
Resolving the setting, and later each edit, writes the
[scope warnings](../selection.md#scopes) to standard error, yellow when
[`supports_color`](../theme.md) allows it:
`Warning: No models match pattern "<pattern>"` for a pattern that selects nothing,
and the invalid thinking level warning for an unknown suffix.

Every edit applies to the session at once but is not saved. A selection with
fewer models than the panel opened with becomes the session scope, in its order
and with any thinking level named in its patterns. Enabling every model, none,
or as many as the panel opened with clears the session scope. The provider count
is refreshed after every edit.

Saving (`app.models.save`) writes the selection to `enabledModels` in its order
and shows `Model selection saved to settings`; the panel stays open. Saving with
every model enabled, or as many as the panel opened with, removes the setting.
An empty or other explicit selection is saved as it is. Escape closes the panel,
keeping the session's edits and leaving the setting as it was.

## Removing stored credentials

`/logout` lists the credentials saved by `/login` in the panel that replaces the
editor. It lists none of the authentication configured elsewhere: environment
variables, runtime keys and `models.json` are not choices and are never touched.
With nothing saved it shows
`No stored credentials to remove. /logout only removes credentials saved by /login; environment variables and models.json config are unchanged.`

Choices are ordered by the catalog's display name for each provider under the
current locale, and providers whose display names compare equal keep the order
they are stored in. A record that does not decode as a complete subscription or
API key is listed by the store but is not offered, because the picker reads
typed credentials only.

Selecting puts the editor back and then removes that one credential from the
current session, refreshes the catalog, updates the footer's provider count and
shows `Logged out of <name>` for a subscription or
`Removed stored API key for <name>. Environment variables and models.json config are unchanged.`
for a key. The name and the kind are the ones the picker listed, so a record that
changes while the panel is open still reports what was chosen. Escape closes the
panel and changes nothing.

A catalog refresh failure shows `Error: Logout failed: <error>` and keeps the
removal already made; nothing is put back. When the catalog cannot say which
providers are available, the footer count becomes zero and the removal still
reports its success. A credential file that cannot be written keeps the removal
in memory, leaves the file as it was and stays a store diagnostic for its own
readers instead of becoming a logout failure.

## Subscription billing notice

When the selected model belongs to the `anthropic` provider and a stored sign-in
or a key starting with `sk-ant-oat` pays for it, the mode shows once:

```text
Warning: Anthropic subscription auth is active. Third-party harness usage draws from extra usage and is billed per token, not your Claude plan limits. Manage extra usage at https://claude.ai/settings/usage.
```

The check runs after a model is applied or cycled. A stored sign-in is enough;
its key is not looked up or refreshed. Other providers, ordinary or missing keys
and failed key lookups show nothing, and a later check may still show the notice.
Setting `warnings.anthropicExtraUsage` to `false` turns the notice off; any other
value leaves it on. The notice is shown at most once for the life of the
interactive mode, even when its session is replaced, and checks that overlap
show it once. Checking at startup comes with the prompt loop.

## What is missing

- Import (`/import <path.jsonl>`) is not delivered. It waits for the shared
  dialog host and the session rebuild helpers, and it stays part of the same work.
- The complete offline viewer is separate work; the export tests supply a
  controlled bundle, so they prove the serializer wiring and not a shipped viewer.
- Packaging and asset injection are separate work. The mode reads the bundle the
  configuration's package directory selects, exactly as the other export callers
  do.
