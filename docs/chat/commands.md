# Interactive commands

The interactive mode reads a few commands from the editor before ordinary
input. Submitted text is trimmed with the shared whitespace rule first: BOM and
no-break space count as whitespace, while U+0085 and zero-width space do not. The
editor empties itself before calling back; a slash command never answers the
waiting input and never enters the editor history. Any other text, including
`/Model`, `/models`, `/model<Tab>term`, `/scoped-models extra`, `/login extra` and
`/logout extra`, is ordinary input.

The mode handles shell input and the slash commands `/export`, `/import`, `/changelog`,
`/model`, `/thinking`, `/scoped-models`, `/settings`, `/login`, `/logout`, `/name`, `/session`,
`/debug`, `/hotkeys`, `/share`, `/copy`, `/fork`, `/clone`, `/tree`, `/resume`, `/new`,
`/compact`, `/reload` and `/quit`. The offline viewer and packaged asset injection are not
delivered yet (see
[what is missing](#what-is-missing)).

`maestro_chat::InteractiveMode` exposes the bodies of the session commands as
methods. Each method reads the runtime's current session when it is called, so it
follows a session switch.

Editor submission routes `/name` or `/name ` followed by a name, `/session`,
`/hotkeys` and `/debug` to those methods. Other text, such as `/names` or
`/session extra`, is ordinary input. When a command fails, the failure goes to
the terminal host.

Extensions use the same mode through the [extension UI](extension-ui.md): dialogs
restore the current editor when they settle, completion wrappers rebuild at once,
and notifications use the transcript's status, warning and error rows.

## Forking and cloning

`/fork` and the configurable `app.session.fork` action open the earlier-message
panel. It lists user messages in chronological order, with the newest selected.
Enter forks before the selected message, replays the new session and puts that
message's text in the editor. The panel closes with `Forked to new session`.
With no user messages, the mode shows `No messages to fork from` instead.

Escape closes the fork panel, restores editor focus and keeps the draft that was
there when the panel opened. This draft restoration differs from a successful
fork, which replaces the draft with the selected message's text. An extension
can veto a fork; that also closes the panel and keeps the draft.

`/clone` copies the current branch through its exact leaf into a new session.
After replacement callbacks finish, it replays that session, clears the editor
and shows `Cloned to new session`. Without a leaf, it shows `Nothing to clone yet`.
Cloning has no Escape cancellation; an extension can veto it. A veto leaves the
session and transcript alone. A submitted command has already cleared the editor.
Fork and clone failures show an error rather than a success notice.

## Session tree

`/tree` opens the current session's tree at its current leaf, using the saved tree
filter. The configurable `app.session.tree` action opens the same panel. Selecting
another entry offers `No summary`, `Summarize` and `Summarize with custom prompt`.
The `branchSummary.skipPrompt` setting skips this question and navigates without a
summary. Selecting the current leaf shows `Already at this point` instead.

Custom instructions may be empty. Cancelling their editor returns to the summary
choices; cancelling the choices reopens the tree at the same entry. A summary
shows a loader whose interrupt key cancels it. Cancelling a summary reopens that
entry, while an extension veto closes navigation without changing the transcript.

Successful navigation replays the selected branch and delivers input captured
during compaction. Returned message text fills an empty or whitespace-only editor;
an existing draft is kept. The panel's label action records or removes the selected
entry's label through the session manager.

Pressing the interrupt key twice on an empty or whitespace-only editor opens the
tree or the earlier-message fork panel, as the `doubleEscapeAction` setting says;
`none` turns this off. The second press must come strictly less than 500 ms after
the first, and the setting is read on each press. A streaming response, a running
shell command and shell mode take the interrupt key first.

## Resuming a session

`/resume` and the configurable `app.session.resume` action list the saved sessions
of the current project; the selector's scope key lists every project instead.
Choosing one closes the panel and resumes it through the runtime, then replays it
and shows `Resumed session`. An extension can cancel the switch, which changes
nothing. Cancelling the selector puts the editor back, its exit key shuts the mode
down, and its rename action records a trimmed, nonblank name in the chosen file.

When the saved session's directory no longer exists, the mode asks whether to
resume in the current directory instead. Declining shows `Resume cancelled`;
accepting retries there and shows `Resumed session in current cwd`. A failed retry
goes to the terminal host. Any other failure to resume is fatal: the mode shows
`Failed to resume session: <reason>` and the process exits with status 1.

## `/quit`

After the shared trim, exact `/quit` clears the editor and starts graceful
shutdown before input, queue or history delivery, without requesting an extra
frame. It also works while a response streams or the session compacts. `/QUIT`,
`/quit extra` and `/quit` followed by U+0085 remain ordinary input.

## `/new`

After the shared trim, exact `/new` clears the editor and starts a new session.
The `app.session.new` editor action does the same but leaves the editor's text
alone. It has no default key: bind one in `keybindings.json`, and a live
remapping takes effect at once. `/NEW`, `/new extra` and `/newer` remain
ordinary input.

The command first stops working progress and empties the status rows. An
extension can veto the switch through `session_before_switch`. A vetoed request
keeps the session, its transcript, its queued input and its pending shell rows,
and shows nothing.

Once the new session starts, the mode clears the transcript, the pending area,
the input queued during a compaction and the shell rows still waiting to move.
It then replays the new session and shows `✓ New session started` in the accent
colour. A shell command from the old session never reappears later.

If the session cannot be created or replayed, the mode shows
`Error: Failed to create session: <failure>`, stops the terminal and exits with
status 1.

## `/compact`

After the shared trim, `/compact` alone or followed by a space and instructions
compacts the session. The instructions are trimmed with the same whitespace rule
and passed to the compaction as written, inner spaces included; a command with
nothing after the space passes none. `/COMPACT`, `/compactx` and `/compact`
followed by a tab, a newline or a no-break space remain ordinary input.

The command counts the stored message entries of the whole session, including
those outside the selected branch; other entries, such as custom messages and
labels, do not count. With fewer than two, it shows
`Warning: Nothing to compact (no messages yet)` and leaves working progress
alone. Otherwise it stops working progress, empties the status rows and starts
the compaction.

The compaction's own events show what happened: a summary row on success,
`Error: Compaction cancelled` when an extension cancels it, or
`Error: Compaction failed: <failure>`. Input submitted while it runs is delivered
to the session once it ends. The command adds no message of its own.

## `/reload`

Exact `/reload` clears the editor and reloads keybindings, extensions, skills,
prompts and themes. `/reload extra` is ordinary input. A streaming response or a
compaction refuses reload with a warning; a running shell command does not.

The command resets extension UI, cancels abandoned dialogs and paints a bordered
loading box before awaiting the application reload. It then refreshes the header,
themes, editor and terminal settings, completion, extension shortcuts and the
transcript, before restoring the current editor with focus. Resource diagnostics
and model-file errors appear even with quiet startup, followed by
`Reloaded keybindings, extensions, skills, prompts, themes`.

A failure after the box appears restores the editor captured after UI reset and
shows `Reload failed: <reason>`. Changes made before the failure remain applied;
the command does not roll them back. A reset failure goes to the terminal host
instead, before application reload starts.

## Extension command contexts

Extension commands receive the interactive mode's session controls. Creating a
session stops working progress and clears the status area; a successful creation
replays the new session without the `/new` command's notice. Forking replays the
new session, replaces the draft with the selected text (or empty text), and shows
`Forked to new session`. An extension veto leaves the transcript and draft alone.
A creation or fork failure uses the fatal runtime error route.

Tree navigation forwards the supplied summary controls and label. On success it
replays the selected branch, fills a still-blank editor with nonempty returned
text, shows `Navigated to selected point`, and starts delivery of input captured
during compaction without waiting for that delivery. Cancellation changes no
presentation; a navigation failure returns its original error to the caller.

Switching sessions uses the same resume operation and missing-directory prompt
as `/resume`. Reload uses the same presentation and refusal conditions as
`/reload`, but does not clear the draft first. Waiting for idle reads the current
agent. These callbacks do not retain the mode when inactive; an admitted operation
keeps it alive until it settles, and retired command contexts remain stale.

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

## `/import`

`/import <path.jsonl>` copies a session into the current session directory and
replaces the current session after confirmation:

```text
/import saved.jsonl
/import "notes from friday.jsonl"
```

An absent, empty or unterminated path shows `Usage: /import <path.jsonl>`.
Otherwise the dialog asks `Import session` and
`Replace current session with <path>?`. No or Escape shows `Import cancelled`
without changing the current session or its progress. Yes stops the working
indicator and clears the status area before importing. An extension veto also
shows `Import cancelled`, but occurs after that progress cleanup.

A successful import replays the imported history and shows
`Session imported from: <path>`, using the authored path. A missing stored working
directory opens the existing directory confirmation. Declining leaves the
current session unchanged; accepting retries the same file once with the offered
working directory. The copied file is not rolled back after cancellation.

A missing input file shows `Failed to import session: <diagnostic>` and leaves
ordinary input available. Other first-attempt failures use the fatal error path:
the final error is rendered, the theme watcher and terminal stop, and the process
exits with status 1. A retry failure instead reaches the terminal runtime's error
reporter; it prints the error and keeps running with the input waiter unresolved.
A rejected operation does not perform the command's final editor clear.

The runtime [binds the mode](interactive-mode.md#session-binding) to the imported
session before the replay. When the replay and the status line run, the new
session's UI, settings, shortcuts, footer and window title are already in place, and
events from the replaced session no longer reach the mode.

## `/share`

`/share` exports the current session to HTML and publishes it as a secret GitHub
gist, then reports the gist and, when a hosted viewer origin is configured, its
viewer URL. It takes no argument: `/share extra` is prompt text.

```text
/share
```

The command runs in four steps:

1. It runs `gh auth status`. A program that is missing or cannot be executed shows
   `GitHub CLI (gh) is not installed. Install it from https://cli.github.com/`;
   a nonzero or signalled status shows `GitHub CLI is not logged in. Run 'gh auth
   login' first.` Neither starts an export.
2. It writes the session, through the same HTML export owner `/export` uses, into a
   directory this share operation alone owns.
3. It replaces the editor with a cancellable `Creating gist...` loader and runs
   `gh gist create --public=false <path>`, collecting the command's output.
4. It restores the editor, removes its own directory and document, and reports the
   result: `Share URL: <viewer>` and `Gist: <url>` when a viewer origin is
   configured, `Gist: <url>` when it is not, or `Failed to create gist: <message>`
   in the transcript's error colour. A zero exit whose output holds no identifier
   shows `Failed to parse gist ID from gh output`; the identifier is the text after
   the last literal slash, kept exactly as printed.

Cancelling the loader with the cancel key terminates the gist command, restores the
editor, removes the document and answers `Share cancelled`. A failure to export the
session shows `Failed to export session: <message>` and starts no gist. Every
failure after the editor is replaced restores it, and cleanup never replaces the
result.

Two share operations never write the same path: each creates its own directory, and
a directory that already exists is refused rather than reused. The captured output is
decoded once from the complete bytes, so a multi-byte character split across pipe
reads survives.

## `/copy`

`/copy` writes the last answer to the clipboard. It takes no argument.

```text
/copy
```

The text is the session's last answer, trimmed under the shared whitespace policy
(U+0085 is kept), with its text blocks joined without a separator and thinking
omitted. An answer that was aborted with no content is skipped in favour of the
previous one. With nothing to copy the mode shows `No agent messages to copy yet.`
A completed copy shows `Copied last agent message to clipboard`; a reached
clipboard failure shows its own message in the error colour. The mode reads the
current session each time, so a replaced session is copied from its own answer.

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

## `/hotkeys`

`/hotkeys` appends a bordered "Keyboard Shortcuts" card with Navigation, Editing
and Other tables. The built-in rows read the live display registry when the
command runs, including alternative bindings in their resolved order and empty
labels for disabled actions. Windows adds ` (Ctrl+Enter on Windows Terminal)`
to the New line description.

`handle_hotkeys_command()` uses the same Markdown card renderer as `/changelog`,
with the current session's code-block indentation. The title captures its theme;
the Markdown and borders keep reading the live theme. A theme failure returns
through the existing terminal error route.

After the shared trim, only exact `/hotkeys` is a command. `/hotkey`, `/HOTKEYS`,
`/hotkeys extra` and `/hotkeys` followed by U+0085 remain ordinary input. The
command leaves the waiting input pending and adds no editor history.

An Extensions table follows when the current session has selected shortcuts.
The runner applies the retained key configuration for reservations, independently
of the display registry. Rows keep selection order and use the description, or
the extension path when it is absent; an empty description stays empty.

## How the path is read

The reader recognizes the command token, then reads one path. It is token
reading, not shell parsing:

- The text must be exactly `/export` or `/import`, or that token followed by
  one literal space. A prefix such as `/exporter` or `/important`, or either
  command followed by a tab, is ordinary prompt input.
- After that single space, leading whitespace is removed under the shared
  [whitespace policy](../terminal/text.md): Unicode white space other than
  U+0085, plus U+FEFF.
- A quoted path returns the text between the first quote and the next matching
  quote, including empty text. An unterminated quote leaves no path, so the
  export command writes HTML under the default name; import shows its usage.
- A bare path ends at the first whitespace character. Later tokens are ignored.
- Nothing is unescaped, expanded or normalized: `~/notes.jsonl` and `"notes
  from friday.jsonl"` reach the import or export operation unchanged.

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

## Thinking level

`/thinking` opens the existing thinking-level picker for the current model.
Choosing a level applies it, refreshes the footer and editor border, and shows
`Thinking level: <level>`. Escape closes the picker without changing the level.

`/thinking <level>` applies a supported level directly. Level names ignore case:
`/thinking HIGH` selects `high`. An unknown or unsupported level changes nothing
and shows `Unknown thinking level "<level>". Available levels: <levels>.`, listing
only the current model's levels. A changed level is saved to the session and
becomes the default, just as with the thinking-cycle shortcut or `/settings`.

The command appears in autocomplete as `Set thinking level` with the `<level>`
hint. The command name itself is case-sensitive; `/THINKING`, `/thinkingx` and
`/thinking` followed by a tab and a level remain ordinary input.

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

## Changing settings

`/settings` opens the settings panel in place of the editor over the current
session and preferences: the panel reads the live session's compaction and queue
policies, the model's own thinking levels, the registered and discovered theme
inventory, and the typed preference values. A stored value of the wrong JSON type
counts as unset and the getter's default applies; the stored settings are never
rewritten by a read.

Choosing a value applies it at once and saves it through the settings owner:
session policies go to the session, which saves them, and the rest are written to
the settings file. Image visibility and width, padding, completion height,
thinking, the hardware cursor and the shrink policy update the retained rows,
editor and terminal straight away. Turning skill commands on or off saves the
choice and then installs a rebuilt completion provider on the editor, so the next
completion there offers `/skill:` commands or leaves them out. Thinking visibility
also rebuilds the transcript from the session's messages. Themes publish
immediately; a preview never saves, a commit saves the requested name, and a
theme that fails to load falls back to `dark` with:

```text
Failed to load theme "<name>": <error>
Fell back to dark theme.
```

Every theme choice keeps the supplied watch effects, so a selected custom theme
is watched again after a preview or a commit. Escape closes the panel, restores
the editor with its focus and requests a frame. Padding and completion height
reach the default editor only; an installed custom editor does not receive them
yet.

## Providers offered for API-key sign-in

`maestro_chat::interactive_mode::is_api_key_login_provider` decides whether a
provider can sign in with an API key. A provider with a nonempty shipped display
name always can. Otherwise a shipped catalog provider cannot, and neither can a
provider that signs in only by subscription; any other provider can. A caller may
pass its own set of shipped providers in place of the catalog's. Identifiers are
compared exactly, so `Anthropic` and `anthropic` are different providers.

## Signing in

`/login` replaces the editor with `Select authentication method:` and the choices
`Use a subscription` and `Use an API key`; Escape puts the editor back. A method
opens the provider picker, `Select provider to configure:`, with every registered
subscription or every catalog provider that can sign in with a key. Choices are
ordered by display name under the current locale; equal names keep subscriptions
first and then the catalog's order. Escape in the picker returns to the method
choice. A method with no provider shows `No subscription providers available.` or
`No API key providers available.` instead of an empty picker. Listing the choices
runs no key command.

### API keys

Choosing a key provider opens a dialog that asks `Enter API key:`. The pasted key
is trimmed with the shared whitespace rule, so BOM and no-break space are removed
while U+0085 and zero-width space are kept. An empty key shows
`Error: Failed to save API key for <name>: API key cannot be empty.` Otherwise the
key is saved for that provider on the current session, the editor comes back and
the sign-in completes. Escape cancels without a message. A credential file that
cannot be written keeps the key in memory and leaves the failure as a store
diagnostic, as removal does.

Amazon Bedrock reads AWS credentials, so choosing it shows `Amazon Bedrock setup`
with how to configure them and the path of `providers.md` in the documentation
directory. Closing the dialog puts the editor back; nothing is saved.

### Subscriptions

Choosing a subscription opens its sign-in dialog and starts the provider's sign-in.
The dialog shows the authorization link, opens it in the browser and shows the
provider's instructions when they are not empty. A provider with a callback server
also accepts a pasted redirect under
`Paste redirect URL below, or complete login in browser:`; empty text is ignored,
every request for it sees the same answer, and the provider decides which of the
two arrives first. After an empty submission, another paste or Escape still works.
GitHub Copilot shows `Waiting for browser authentication...`
while it polls. Questions, progress lines and choices appear in the same dialog; a
choice replaces the dialog until it is answered, then the same dialog comes back
and the provider receives the identifier of the first option with the chosen label.

Each callback is shown before it returns, so a callback that cannot be shown
returns that failure to the provider, which ends the sign-in before any credential
is saved. Callbacks the provider keeps after its sign-in ends are refused. Escape
cancels without a message; any other failure shows
`Error: Failed to login to <name>: <error>` after the editor comes back.

### After signing in

The catalog is refreshed. When the session still has no resolved model, the
provider's configured default model is selected. The footer's provider count and
the editor border are updated, then one status line reports
`<action>. Selected <id>. Credentials saved to <path>` or, without a selection,
`<action>. Credentials saved to <path>`. The action is `Logged in to <name>` or
`Saved API key for <name>`, and the path is `auth.json` in the agent directory. When
the default model cannot be selected, an error follows with the reason:

- `<action>, but no default model is configured for provider "<id>". Use /model to select a model.`
- `<action>, but no models are available for that provider. Use /model to select a model.`
- `<action>, but its default model "<id>" is not available. Use /model to select a model.`
- `<action>, but selecting its default model failed: <error>. Use /model to select a model.`

A failed selection keeps whatever the session already changed. Without such an
error, the [subscription billing notice](#subscription-billing-notice) is checked
for the selected or current model without holding up the sign-in. A refresh failure ends
the sign-in with its error instead of a status line.

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

- The complete offline viewer is separate work (#1762); the export tests supply a
  controlled bundle, so they prove the serializer wiring and not a shipped viewer.
  Until it ships, a share with no bundle installed follows the export failure path
  above and reports it.
- Packaging and asset injection are separate work (#249). The mode reads the bundle
  the configuration's package directory selects, exactly as the other export
  callers do.
