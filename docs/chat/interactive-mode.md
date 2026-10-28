# Interactive mode

`maestro_chat::InteractiveMode` holds the native terminal presentation of the
current session. This page covers what exists today: construction, startup,
the startup help, the loaded-resource listing, the transcript notices, typed
input, queued input, the export, import, share, copy and changelog commands, the prompt loop, session replay, the
session events with the working row and the streamed transcript rows, the window
title, the extension controls of that row and of the hidden-thinking label, the
expansion and thinking shortcuts, thinking and model cycling, model selection and
scopes, the editor border, external editing, image paste, compaction and retry
progress and native shutdown. Installed custom headers and the other integrations
are still to come.
[Interactive commands](commands.md) describes the session command methods,
[session-tree navigation](commands.md#session-tree) and
[resuming a session](commands.md#resuming-a-session).

## Construction

`InteractiveMode::new` takes the application's `AgentSessionRuntime`, the
`InteractiveModeOptions`, the terminal and its runtime host, the theme registry
and the footer effects (`FooterOperations`, such as `NativeFooterOperations`). It
reads the current session's settings:

- `showHardwareCursor` and `terminal.clearOnShrink` configure the frame writer;
- `editorPaddingX` and `autocompleteMaxVisible` configure the editor;
- `compaction.enabled` selects the footer's `(auto)` marker.

The constructor loads key bindings from `keybindings.json` in the agent directory
and publishes them as the toolkit's registry. The footer metadata watches the
session's working directory. Last, the themes the session's resource loader has
loaded are registered and the configured `theme` is selected, so a loaded theme
wins over a shipped one with the same name. The setting is read after the loader
answers. An unreadable or malformed bindings file is skipped and the default
bindings apply; a failure to find the agent directory, build the footer's collator
or load the fallback theme is returned. The constructed mode installs its rebinding and its
[extension UI reset](extension-ui.md#session-replacement) on the runtime, both
holding the mode weakly.

## Startup

`init` is asynchronous. It installs termination and output-error handlers, selects
startup history, then prepares both search programs before building the presentation.
It does nothing while the mode is initialized. After `stop`, it runs again.
Unix initialization runs on a caller-driven Tokio local executor with signal
delivery enabled. Registration failure leaves the UI unstarted. It then adds the header, transcript, pending-messages, status,
upper widget, editor and lower widget containers and the footer, in that order. The upper widget container starts with
one blank spacer row; the lower widget container starts empty. It focuses the
editor, binds the transcript shortcuts, the queue keys and editor actions, installs
editor submission and the global debug key, and starts the terminal. A terminal
start failure is returned and leaves the mode uninitialized. Once started, it
[binds the current session](#session-binding), so the loaded resources are listed
before it replays the session (see below). A theme change then
invalidates the presentation, recolours the editor border and requests a frame,
and a branch change requests a frame. Neither callback keeps the mode alive. Last,
the footer learns how many providers the model candidates cover. The history
selection and version recording happen before building the header, even if
starting the terminal later fails.

The search programs are `fd` and `rg`. Both start before either finishes, so a missing
program does not delay the other. `fd` stays selected for file completion. The managed
directory is the application's agent directory under `bin`, never the session's
working directory. The search owner writes its own progress and advice lines, before
the terminal takes the screen.

`InteractiveModeOptions::verbose` forces the expanded startup help, as does
expanded tool output. Without verbose, the `quietStartup` setting leaves the
header empty. Otherwise the header shows the
application name and version, the compact key hints and two onboarding lines. In
verbose mode it shows the full list of key hints instead. Hints follow the loaded
bindings: several keys keep their order, and an unbound action shows no key.
Header text keeps the colours of the theme in effect when it was built.

## Session binding

Binding the current session runs these phases in order:

1. Remove the session subscription, if there is one.
2. Apply the runtime settings.
3. Bind the session's extensions, with the mode as their
   [user interface](#extension-ui-host), and wait for their `session_start`
   handlers and resource discovery. A `session_start` handler can already open a
   dialog, because the terminal has started.
4. Register the loaded themes, rebuild [completion](extension-ui.md#completion) and
   install the [extension shortcut interceptor](extension-ui.md#shortcuts) from the
   current runner.
5. List the loaded resources, with their diagnostics even when startup is quiet,
   and show the [startup history](startup.md#startup-history) once.
6. Subscribe to the session events, refresh the footer's provider count, recolour
   the editor border and set the window title.

Binding returns the first failing phase's error. The phases before it stay
applied, nothing is rolled back and no later phase runs.

The mode registers this binding with the runtime when it is built. Each time the
runtime replaces its session (new session, resume, fork or import), it binds the
mode to the new session before the operation's continuation runs. The continuation
therefore sees the new session's UI, resources, footer and window title. The next
shortcut reads the new runner, and events from the replaced session no longer reach
the mode. The runtime holds the mode weakly: once the mode is gone, replacement skips
the binding. A binding failure is returned to the caller of the runtime operation.

The runtime settings give the footer the current session, its automatic-compaction
marker and its working directory. They then set the thinking visibility, the hardware
cursor, clear-on-shrink, and the editor's padding and visible completion rows, in
that order. Hiding the hardware cursor writes to the terminal; a failed write is
returned, and the settings before it stay applied.

The binding also installs the mode's shutdown and error handlers. An extension's
shutdown request records the request; while the session is idle it starts
[graceful shutdown](#native-shutdown) at once, and while a response streams the
agent's end starts it. A shortcut's context reaches the same handler.

Each attributed extension failure the runner reports is appended to the transcript
as `Extension "<path>" error: <message>` in the error colour. When the failure
carries a non-empty stack, every stack line after the first follows as one dim
block. Each line is trimmed of white space and indented by two spaces, and empty
lines are kept. The mode requests one frame. A colour failure is returned to the
runner.

## Stopping the presentation

`stop` removes lifecycle handlers, clears terminal progress when enabled, stops
and releases the working loader, clears extension terminal input listeners,
disposes the footer and its repository watch, removes the session subscription,
and stops the UI if initialized. Unrelated terminal listeners stay registered.
The status container keeps its
rows. A later session event no longer restarts this presentation, and the shared
theme watch stays active.

A progress-clearing failure prevents the remaining cleanup, leaving extension
input listeners active. A UI-stop failure leaves them cleared and the mode
initialized; only a successful UI stop clears that flag. Reinitialization does
not restore cleared extension callbacks. The original I/O error is returned.
Repeating a successful stop does not stop the
absent loader or UI again. This operation does not dispose the application
runtime, terminate the process or kill detached children. The registered `app.clear` action clears the editor and requests a frame. A second
press less than 500 ms after the last clear starts graceful shutdown; exactly
500 ms clears again. Only the clear branch updates that wall-clock timestamp.
`app.exit` starts the same shutdown only when the editor is empty. Both actions
honor configured bindings (by default Ctrl+C and Ctrl+D), preserve editor history
and do not add signal-only detached-child cleanup.

## Native shutdown

On Unix, TERM kills tracked detached children, then starts graceful shutdown. The
mode removes its handlers and drains terminal input with the existing 1,000 ms
maximum and default 50 ms idle period. It stops the presentation, awaits application
disposal and exits 0. Admission happens before asynchronous work is scheduled;
repeated requests do not repeat shutdown. A drain, progress, UI-stop or disposal
failure ends that sequence without a successful exit.

HUP and native stdout/stderr disconnection errors take the emergency route:
remove handlers, kill tracked children and exit 129, without draining, restoring
the terminal or disposing the application runtime. EIO, broken-pipe and
not-connected errors select this route; input errors and other output errors
keep their existing handling. During graceful disposal, once the mode's signal
handlers are removed and no other registration in the process is live, a later
termination or hangup signal takes its native default action. Windows
console-close integration is not provided by this mode.

At agent end, a requested shutdown follows transient-row cleanup through this
same graceful owner instead of requesting a final frame. Extension requests reach
it through the [session binding](#session-binding).

## Loaded resources

The listing appends one section per kind of loaded resource that has entries, in
the order `[Context]`, `[Skills]`, `[Prompts]`, `[Extensions]`, `[Themes]`. Each
section is its heading over its entries, followed by an empty row; `[Context]` also
gets an empty row above it.
With `quietStartup` set and verbose off, startup lists no sections but still shows
the resource diagnostics described below.

`[Context]` lists the context files the session's resource loader discovered, in
discovery order. The compact form puts them on one row, separated by commas: a file
inside the working directory shows its relative path (`.` for the directory
itself), and any other file, including one whose relative path would start with
`..`, keeps its full path. The compact form resolves paths lexically, without
reading the filesystem; the expanded form puts each path on its own row in the
spelling the loader supplied. A leading home directory is shown as `~`; a home
directory that is empty or `/` abbreviates nothing, so paths stay as written. The
home directory is looked up whenever a listing prepares a path that shows it
abbreviated. Every section prepares its expanded body even while it shows only its
compact form, so a listing of context files, or of skills outside a package, needs
the lookup when only names or relative paths show. An empty listing or one of
prompts alone needs none. When that lookup fails, the listing reports the failure.

`[Skills]` and `[Prompts]` list the loader's skills and the session's prompt
templates. The compact form is one row of skill names, or of prompt names with a
leading `/`, trimmed and sorted for the current locale. The expanded form groups
entries under `project`, `user` and `path` (command-line and temporary resources).
Within each scope it lists plain paths first, then each package source with its
entries, all sorted for the locale. A package entry is shown relative to its
package root, or else by its path inside the npm or Git package directory, or
else by its full path with the home directory shown as `~`. Prompts
show their `/name`; when several templates share a file, the last one's name is
shown.

`[Extensions]` lists the loaded extensions. The compact form is one row of sorted
labels. A package extension is labelled by its package: an npm source without
`npm:`, or a Git source by its repository path. An index file below the package's
`extensions` folder adds its folder after a colon (`workers:subagents`), and any
other file adds its path. Other extensions are labelled by the shortest end of
their path that no other such extension shares: `index.ts` and `index.js` files
show their folder, and parent folders are added only as needed, so
`/a/tools/index.ts` and `/b/tools/index.ts` become `a/tools` and `b/tools`. Two
extensions at the same path keep the same full label. Package labels never make
other labels longer. The expanded form groups the extensions by scope and package,
showing each path without a final `/index.ts` or `/index.js`.

`[Themes]` lists the loaded themes that came from a file; themes without a file,
such as the shipped ones, are left out. The compact form is one row of theme names,
sorted like skill names. A theme without a name shows the last segment of its path
instead, and an empty name is left off that row. The expanded form groups the
theme files by scope and package, as for skills.

After the sections come the resource diagnostics, as plain text that does not
expand: `[Skill conflicts]`, `[Prompt conflicts]`, `[Extension issues]` and
`[Theme conflicts]`, each heading in the warning colour and followed by an empty
row, and each left out when it has nothing to show. Name collisions come first,
grouped by name in the order first seen: `"name" collision:`, the kept file after
`✓`, then each skipped file after `✗` with `(skipped)`. Other diagnostics follow
in order, in the error colour for errors and the warning colour otherwise: a
diagnostic with a path shows the path on one row and its message indented below
it, and one without a path shows only the message. `[Extension issues]` lists the
extensions that failed to load, the command and shortcut problems the extensions
reported, and each extension command named like a built-in command, which is
either skipped in completion or reachable under its numbered name:

```text
Extension command '/settings' conflicts with built-in interactive command. Available as '/settings:1'.
```

A path in a diagnostic is shown after the source of the listed resource at that
path, or else at its nearest parent: `user`, `project` or `path (temp)` for local
files, `path` for command-line files, and a package source with its scope, such as
`npm:tools (project) extensions/index.ts`. A path with no known source is shown with
only its home directory abbreviated.

Sections start expanded in verbose mode or when tool output is expanded, and compact
otherwise. Each section keeps the text it was built with. Expanding or collapsing it
later repaints only its heading in the current theme.

## Notices and the editor

`show_error` and `show_warning` append an empty row and a padded notice, labelled
`Error:` or `Warning:` in the theme's error or warning colour, to the transcript.
Each requests one frame and returns the published theme's error, so the caller
keeps the original failure type. The supplied text is kept as given, including line
breaks and its own styling. `clear_editor` empties the editor and requests a
frame; the editor's history is kept.

## Extension UI host

The mode implements the extension UI port, and `init` installs it as the
session's extension UI. The session and its runner only borrow the mode, which
owns them, so they do not keep it alive; a UI context an extension
holds keeps the mode alive until it is dropped, and afterwards the session's
contexts report no UI. Another host retains it with
`ExtensionUIContext::new(mode.clone())`; the [extension UI page](extension-ui.md)
lists the operations it answers today, which are dialogs, editor access, theme
queries and selection, the terminal title, tool expansion, status text, keyed
widgets above or below the editor, extension headers and footers, custom
editors and custom views. Notifications and terminal input listeners
keep their own published owners.

## Typed input

`get_user_input` returns a future for the next text the user submits from the
editor, with the key bound to `tui.input.submit` (Enter by default). Submitted
text loses its outer whitespace under the shared whitespace policy: Unicode white
space other than U+0085, plus U+FEFF. U+0085, U+200B, inner spaces and line breaks
stay. Submission ignores text that is empty after trimming: no input receives it
and the history does not record it. The session commands in
[Interactive commands](commands.md) are handled before this step. While the session
compacts or a response streams, submitted text is queued instead (see Queued
input) and no input receives it.

Only one input waits at a time. The future starts waiting when it is first
polled, not when it is created, and it does not borrow the mode. When it waits,
the next accepted submission completes it and clears the wait. The mode keeps no
submission for a later input. A later future replaces the waiting one when it is
polled, and the replaced future stays pending.

The waiting input receives the text before the editor's history records it, so
code that runs when the input completes can already wait for the next one. Every
submission delivered as prompt text enters the history, whether or not an input
was waiting. A prompt submission does not keep the mode alive. An admitted
asynchronous command retains it until that operation settles.

Submission offers the text to the mode's commands first. Text recognized as one of
the commands runs that command instead of becoming prompt text: the command reports
on the transcript, completes no waiting input and enters no history. `/export`,
`/changelog` and `/hotkeys` answer before the editor hands the text back; `/import`,
`/share` and `/copy` are admitted on the terminal runtime. Import clears the editor
only on normal completion, leaving text written during a rejected operation intact.
Import confirmation and directory retry reuse the native dialogs, and its
replacement is [bound](#session-binding) before the replay. The submission is trimmed
first, so `/export` followed only by a tab runs the command and a submission of white
space alone is dropped. Near spellings are prompt text, so `/exporter notes.jsonl`,
`/export<Tab>path`, `/share extra`, `/important`, `/import<Tab>path` and `/copyx`
reach the input. See [interactive commands](commands.md).

## Running the prompt loop

`run` awaits initialization before starting the [independent startup checks](startup.md#independent-startup-checks),
then shows the startup warnings, sends the startup prompts and keeps sending typed prompts. It never finishes on its own. It returns only an
initialization failure or a failure to show a notice.

The synchronous startup warnings come in this order, before any prompt:

- a warning `Migrated credentials to auth.json: ` followed by
  `migrated_providers` joined with `", "`, when the list is not empty. Names keep
  their order, duplicates and empty names;
- an error `models.json error: ` followed by the current session's catalog load
  error, when there is one;
- `model_fallback_message` as a warning, when it is not empty. Text made only of
  white space is still shown.

After the fallback notice and before the first prompt, the mode starts the
subscription-auth warning check. It uses the same once-only warning owner as model
selection and sign-in. Prompts do not wait for that asynchronous check to finish.

The first prompt is `initial_message`, sent unless it is absent or empty. Startup
text is never trimmed. `initial_images` go with that first prompt only, and they are
not sent when there is no first prompt. Absent and empty image lists stay distinct
for the session's input handlers. `initial_messages` follow in order, each sent once,
empty and repeated text included. Then the loop waits for typed input with
`get_user_input` and sends each submission.

Every prompt goes to the runtime's session at the time it is sent, so after the
runtime replaces its session, the next typed prompt reaches the new one. When the
session refuses a prompt, such as one sent without credentials, the mode shows the
session's message once as an error and goes on to the next prompt. An answer that
fails after the prompt was accepted stays a session event; the mode adds no notice
for it.

## Session replay

`render_initial_messages` appends the session's current context to the transcript and
clears nothing first; the rebuild after the thinking shortcut clears the transcript and
then replays the same way. Each message is shown by its existing component:

- user text, with its text blocks joined without a separator; a skill block shows
  as a collapsible skill row followed by any text after it as a separate user row;
- assistant content, with thinking shown or replaced by its label;
- one row per tool call, completed by its result when the result follows;
- shell runs, displayed custom messages (through an extension's renderer when one is
  registered for the custom type), branch summaries and compaction summaries.

Hidden custom messages add nothing and never reach a renderer. Tool results add no
row of their own. A call of an aborted message shows `Operation aborted`, or
`Aborted after 1 retry attempt` / `Aborted after N retry attempts` while the
session is retrying; a call of a failed message shows the error text, or `Error`.
Calls without a result stay registered by call id so a later completion can find
them; completed calls are not kept. Tool rows use the configured image visibility
and width, the session's registered renderers for the tool's name and the built-in
renderers of a built-in name. Stored tool arguments and result details keep their
JSON value, so a scalar or array argument is shown as stored.

The initial replay also recolours the editor border, adds each user text to the
editor history and, when the session was compacted, shows `Session compacted 1
time` or `Session compacted N times`. A rebuild after the thinking shortcut replays
without either effect, so the history is not filled twice.

## Queued input

While a response streams, Enter queues the trimmed text as steering and the key
bound to `app.message.followUp` (Alt+Enter by default) queues it as a follow-up.
Both record the text in the history and empty the editor, then send it to the
session as a prompt with that delivery, so registered commands still run at once and
templates still expand. The prompt starts at once; once it is queued, the pending
rows are refreshed. When the session refuses it, the editor stays empty, the history
keeps the text, the rows are not refreshed and the failure goes to the terminal
runtime's error route. With no response streaming and no compaction, the follow-up
key empties the editor and submits the text as Enter does; without an installed
submission it does nothing. The follow-up key reads the editor's expanded text, so a
large paste is sent in full.

While the session compacts, a registered command (`/name`, with the name ending at the
first space) still runs at once. The mode keeps any other text in the order it was
typed, as steering from Enter or as a follow-up from the follow-up key, records it
in the history, clears it from the editor, shows it in the pending rows and confirms
it with the status line `Queued message for after compaction`. When the compaction
ends, the mode delivers what it kept (see
[Compaction and retry progress](#compaction-and-retry-progress)); until then the
messages stay in the pending rows, and the dequeue key restores them.

Queued messages, including those kept during a compaction, appear above the
editor. Each steering message gets a dim `Steering: <text>` row and each follow-up
message a dim `Follow-up: <text>` row, cut to one line. A last row names the keys bound to
`app.message.dequeue` (Alt+Up by default) with `to edit all queued messages`. The
keys are read when the rows are built, each part capitalized, as in `Ctrl+Alt+Q`.
With nothing queued, the rows are removed.

A shell row shown in that area while a response streams stays above those text
rows. Rebuilding them re-adds the same row rather than a new one, so a refresh, a
dequeue or a theme change leaves its identity and its output in place; the next
ordinary submission moves it into the transcript.

The dequeue key moves every queued message back into the editor: steering messages
first, then follow-up messages, separated by blank lines and followed by the current
draft when that draft is not blank. Duplicates and their order are kept. A status
line then says `Restored 1 queued message to editor`,
`Restored <n> queued messages to editor` or `No queued messages to restore`; with
nothing queued the editor is left as it was.

The interrupt key (Escape by default) takes the first case that applies: while a
response streams it restores the queued messages the same way and then stops the
response, even when nothing was queued; while a shell command runs it cancels that
command and keeps the editor text; while the editor holds a shell command it empties the
editor, leaves shell mode and repaints the border for the current thinking level. With
none of those it does nothing. The queue keys do not keep the mode alive, and a failure
in them goes to the terminal runtime's error route.

## Session events

One listener carries the session's events into the mode. It is the session's own
subscription, so the removal operation the session returns is kept for the
lifecycle that ends it. The listener keeps only a weak mode handle. Events arriving
before initialization completes wait in arrival order for one continuation on the
supplied runtime. Once that queue is drained, delivery is synchronous and borrowed:
the mode handles each event before the producer returns. A failure goes to the
terminal runtime's error route; a failed event does not discard later events. A
failed initialization releases its queued batch, and a later event tries again.

Every event then invalidates the footer, which the footer does not cache. The events
that carry session metadata read the session's live state rather than the values
the event carries:

- a changed session name writes the window title;
- a changed thinking level recolours the editor border for the shell colour or
  the new level, as the shortcut section describes;
- a queue update rebuilds the pending rows from the session's queues and the
  messages kept during a compaction, whatever the event lists.

The agent's events drive the working row and the transcript:

- an agent start forgets the registered tool calls, shows terminal progress when
  `terminal.showTerminalProgress` is set, and replaces the working row in the status
  rows, unless an extension hid it: the indicator in the accent colour beside the
  muted working message, painted by the theme in effect when the row last updated
  its frame or message;
- a message start adds a custom or user message to the transcript, the user one
  also rebuilding the pending rows; an assistant message starts one streaming row
  that keeps the message it shows; a tool result adds nothing;
- an assistant update shows the new message in the streaming row and gives each of
  its tool calls a row registered by call id, in the order the calls appear; a call
  that already has a row gets the new arguments in that row. Without a streaming
  row the update is ignored;
- an assistant end shows the ended message. An aborted message gets
  `Operation aborted`, `Aborted after 1 retry attempt` or `Aborted after N retry
  attempts` as its error, written into the message itself, and its registered calls
  complete with that text as an error; a failed message's calls complete with its
  error text, or `Error`. Otherwise the registered calls are marked as having their
  full arguments. The streaming row is then released. A user message's end
  changes nothing and requests no frame;
- a tool start marks its row started, adding a registered row first when the call
  has none; a row the replay or an update made keeps its arguments. A tool update
  shows its output as partial and never as a failure; a tool end shows the final
  output with the event's failure flag and forgets the call. An update or end of an
  unknown call adds no row and requests no frame. The result details reach the
  row's renderer as the tool returned them, with a missing value kept apart from
  null;
- an agent end clears terminal progress when `terminal.showTerminalProgress` is set,
  stops the working row and empties the status rows when a working row is shown,
  removes an unfinished
  streaming row and forgets the registered calls; completed rows stay. A requested
  shutdown then runs through the graceful owner; otherwise the mode requests a frame.

Rows change in place, so an incremental frame rewrites only from its first changed
row. A resize can force a full redraw instead.

Extensions control the working row and the thinking label through the extension UI
host:

- the working message replaces `Working...`, also in a running row; clearing it
  restores the default, while an empty message stays empty;
- hiding the working row stops and removes it, and showing it again while a
  response streams starts a new one; the setting holds for later responses;
- a working indicator replaces the toolkit's own in the running row and in later
  ones: its frames are shown as given, an empty list hides the indicator, and one
  frame stays still; clearing it restores the toolkit's indicator;
- the hidden-thinking label replaces `Thinking...` on every assistant row of the
  transcript, the streaming row and later rows; clearing it restores the default.

The window title is `Maestro - <name> - <directory>` when the session has a name
and `Maestro - <directory>` when it has none. The directory is the last segment of
the session's working directory as authored text, so it needs no filesystem read
and keeps the separators of the platform the build targets. The name is the one
the session stores: the newest name record wins, an empty record clears it, and
the name loses its outer whitespace under the shared whitespace policy. A name
record that holds anything but text or null is an invalid-data failure, which
takes the event's error route and leaves the current title in place.

## Compaction and retry progress

`compaction_start` shows a loader in the status container and makes the interrupt
key abort the compaction. The label names the reason it records: `Compacting
context... (escape to cancel)` when the caller asked for it, `Auto-compacting...
(escape to cancel)` at the reserve threshold, and `Context overflow detected,
Auto-compacting... (escape to cancel)` after an overflow. The parenthesised keys
are the ones bound to `app.interrupt` when the compaction starts, joined with `/`;
an action bound to no key reads `( to cancel)`. The editor keeps its text, its
focus and its submission, so input typed during a compaction still queues. The
terminal's indeterminate progress follows the `showTerminalProgress` setting, read
when the event arrives rather than when the mode was built, so changing it between
the start and the end changes what the next event does.

`compaction_end` restores the interrupt callback the compaction replaced, or the
editor's ordinary action when it had none, turns the terminal's progress off, stops
the loader and empties its row. It then shows what the compaction ended with:

- an aborted manual compaction shows `Compaction cancelled` as an error notice; an
  aborted automatic one shows `Auto-compaction cancelled` as a status line;
- a result clears the transcript, replays the session from the stored context, and
  appends one synthetic compaction summary carrying the summary text, the token
  count and the current time, then invalidates the footer;
- a failed manual compaction shows the supplied message as an error notice; a
  failed automatic one appends it unpainted by that prefix, in the error colour;
- an end with neither a result nor a non-empty failure message adds no row.

`auto_retry_start` shows `Retrying (<attempt>/<limit>) in <seconds>s... (escape to
cancel)` in the status container, where the same key aborts the retry. The
seconds are the wait rounded up, which the existing countdown timer writes on its
first report; it then reports each whole second left, and the label is rebuilt on
every tick so it reads the keys bound then. The attempt limit keeps its numeric
spelling, so a fractional limit reads `2.5`. `auto_retry_end` stops the countdown
and the loader, empties their row, restores the interrupt callback, and shows
`Retry failed after <attempt> attempts: <error>` only when the retries are
exhausted, with `Unknown error` for an absent or empty failure.

A started response clears the retry row and its countdown before the working
presentation resumes, and turns the terminal's progress on. Because the
retry's own row is removed there, a retry end that arrives afterwards removes
nothing. When the response ends, the terminal's progress is turned off again, and
both ends read the setting as their event arrives.

When a compaction ends, the mode captures the input it kept before the session
does anything with it, shows the pending rows without it, and delivers it on the
terminal host. A retried compaction queues each message the way its own kind asks:
a registered command still runs as a prompt, and other text joins the steering or
follow-up queue. Otherwise the first message that is not a registered command starts
the next turn, the commands selected before it prompt first, and the rest of the
batch follows behind that turn in order. Selected commands still prompt if an earlier
command removes their registration; a batch containing only commands prompts them all.
A tail message is not sent once the first prompt has failed. A phase that fails
clears the session's queues, restores the captured batch ahead of any input
admitted since, and shows one notice reading
`Failed to send queued message<s>: <error>`; a second failure changes nothing. A
compaction that ends with nothing captured delivers nothing.

## Status lines

A status is a dim line in the transcript. When the previous status line is still
the last row, a new status replaces its text; otherwise it starts a new line.

## Session replacement and fatal errors

`render_current_session_state` clears the transcript and the pending-message rows,
discards queued compaction input and the active streamed message, then replays the
current session with the initial replay's effects: it refreshes the
footer and the editor border, adds each user text to the editor history and shows the
compaction notice. A transcript row or pending tool row left by the replaced session
does not survive the call, so the presentation shows only the session it now reads.

`handle_fatal_runtime_error` takes a prefix and the error that ended a session
operation. It appends `Error: <prefix>: <message>` to the transcript, where the message
is the error's own text, draws that frame at once, then closes the theme watch and
stops the terminal, so no theme reload or further frame repaints once the notice is
drawn. It then ends the process with status 1. A failure while appending or drawing
the notice, or while stopping the terminal, is ignored, because the process is ending
either way. The notice is drawn before the terminal stops, because a stop cancels a
queued frame and the final frame is not deferred while the terminal's output is backed
up.

## Shortcuts

The expand shortcut (`app.tools.expand`) flips expansion of the startup help and of
every expandable transcript row: loaded-resource sections, tool rows, shell runs,
summaries, skill rows and custom messages. The walk goes through the child array the
transcript holds when it starts, one position at a time: a row a renderer appends to
that array is reached when its position comes up, and a row it removes before then
is not. Replacing or clearing the transcript during the walk does not stop it, which
finishes over the array it started with.

The thinking shortcut (`app.thinking.toggle`) flips and saves `hideThinkingBlock`,
replays the transcript, puts a streaming row back after it with the new visibility
and shows `Thinking blocks: hidden` or `Thinking blocks: visible`. A shortcut's failure goes to the terminal runtime's error route.

Extension shortcuts run before these actions; [extension UI](extension-ui.md#shortcuts)
covers their selection, dispatch and failure notice.

## Thinking and model cycling

The thinking cycle key (`app.thinking.cycle`, Shift+Tab by default) selects the
session's next thinking level. It shows `Thinking level: <level>` as a status line
and refreshes the footer and editor border; when the model cannot reason it shows
`Current model does not support thinking` and changes nothing else. A recording
failure goes to the terminal runtime's error route. `/thinking` opens the level
picker and `/thinking <level>` sets a level directly; see
[the thinking level command](commands.md#thinking-level).

The model cycle keys (`app.model.cycleForward` and `app.model.cycleBackward`,
Ctrl+P and Shift+Ctrl+P by default) select the next or previous scoped model, or
the next or previous available model when none are scoped. The cycle runs on the
terminal runtime, which keeps the mode until it completes. It then shows
`Switched to <name>`, using the model id when the name is empty and adding
` (thinking: <level>)` for a reasoning model whose level is not `off`, and refreshes
the footer and editor border, then checks the
[subscription billing notice](commands.md#subscription-billing-notice). With no
other candidate it shows `Only one model in scope` or `Only one model available`. A
session error is shown as an error notice instead of the switch message. A failed
credential lookup leaves the selection as it was; a recording or listener failure
happens after the new model is selected, so that model stays selected.

`/model`, `/scoped-models`, `/login`, `/logout` and the model-select key are described in
[interactive commands](commands.md).

## Editor border

The editor border shows the input mode. Text whose first character after leading
whitespace is `!` is a shell command, and the border takes the theme's `bashMode`
colour. Leading whitespace follows the same policy as submitted text, so U+FEFF and
a no-break space are skipped while U+0085 and U+200B are not. Otherwise the border
takes the colour of the session's thinking level, `thinkingOff` through
`thinkingXhigh`. The border is repainted and a frame requested only when an edit
switches between the two modes. A failure to read the theme is handed to the
terminal's runtime host, which reports it.

## External editing

The key bound to `app.editor.external` (Ctrl+G by default) edits the editor text in
an external editor. The mode reads the runtime's `VISUAL`, then `EDITOR`, each time
the key is pressed; an empty value counts as unset, and neither value is trimmed.
With no editor configured, the transcript shows the warning
`No editor configured. Set $VISUAL or $EDITOR environment variable.` and nothing
else happens. On Unix the command splits on ASCII spaces only, and the file path is
its last argument; Windows runs it through the configured command interpreter, as
the extension editor dialog does.

The text written is the current editor's expanded text, so a collapsed paste is
written in full rather than as its marker. It goes to `maestro-editor-{milliseconds}.maestro.md` in
the temporary directory. The mode then stops the terminal and runs the editor on
the inherited terminal. A zero exit replaces the text of the editor current at that
moment with the file's text less one final line feed; a nonzero exit, a signal or a
failed launch keep the text.
After every outcome the mode tries to remove the file it created, ignoring a failed
removal, and restarts the terminal with a full redraw. Other failures are handed
to the terminal's runtime host.

## Image paste

Pasting an image (`app.clipboard.pasteImage`: Ctrl+V by default, Alt+V on Windows)
reads the clipboard through the application's
image reader. An image is written to `maestro-clipboard-{UUID}.{extension}` in the
temporary directory, with the extension of its type or `png`, and the file path is
inserted at the cursor. The file stays for the prompt that uses it. No image, a
failed read and a failed write all leave the editor as it was, without a message.

Both actions create their temporary file exclusively, readable only by its owner on
Unix: an existing file or symbolic link of the same name is left untouched, and the
edit stops with an error before the editor runs. Neither action keeps the mode
alive; a clipboard read in progress keeps the mode until it completes.

## Suspend

The key bound to `app.suspend` (Ctrl+Z by default) sends the mode to the
background. From the key press until the process continues, interrupts are
ignored. The mode stops the terminal and sends the stop signal to its process
group. When the shell continues the process, interrupts take their default action
again and the mode restarts the terminal once, with a full redraw. If stopping the
terminal or signalling the group fails, interrupts take their default action again,
a later continue signal does nothing, and the failure is handed to the terminal's
runtime host. The continue signal is observed on the terminal's Tokio runtime,
which must have I/O enabled. On Windows the key only shows
`Suspend to background is not supported on Windows`.

## Startup dispatch contract

`InteractiveMode` implements the application-owned
`presentation_data::mode_dispatch::InteractiveModeRunner`: initialization, run
and stop delegate to the same retained mode. `InteractiveModeOptions` is defined
at that application seam and remains available from both existing chat paths.
This contract does not add another lifecycle or wire an executable.
