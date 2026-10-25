# Interactive mode

`maestro_chat::InteractiveMode` holds the native terminal presentation of the
current session. This page covers what exists today: construction, startup,
the startup help, the loaded-resource listing, the transcript notices, typed
input, queued input, the export command, the prompt loop, session replay, the
session events with the working row and the streamed transcript rows, the window
title, the extension controls of that row and of the hidden-thinking label, the
expansion and thinking shortcuts, thinking and model cycling, the editor border,
external editing and image paste. Installed custom headers, compaction and retry
progress, a requested shutdown and the other integrations are still to come.

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
answers. A failure to read the bindings, build the footer's collator or load the
fallback theme is returned.

## Startup

`init` runs once; later calls change nothing. It builds the header, then adds the
header, transcript, pending-messages, status, upper widget, editor and lower widget
containers and the footer, in that order. The upper widget container starts with
one blank spacer row; the lower widget container starts empty. It focuses the
editor, binds the transcript shortcuts, the queue keys and editor actions, installs
editor submission and starts the terminal. A terminal start failure is returned and leaves the mode
uninitialized. Once started, it subscribes to the session events and sets the window
title. A theme change then invalidates the presentation, recolours the editor border
and requests a frame, and a branch change requests a frame. Neither callback keeps
the mode alive. Last, it lists the loaded resources and their diagnostics in the
transcript and then replays the session (see below).

`InteractiveModeOptions::verbose` forces the expanded startup help, as does
expanded tool output. Without verbose, the `quietStartup` setting leaves the
header empty. Otherwise the header shows the
application name and version, the compact key hints and two onboarding lines. In
verbose mode it shows the full list of key hints instead. Hints follow the loaded
bindings: several keys keep their order, and an unbound action shows no key.
Header text keeps the colours of the theme in effect when it was built.

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
itself), and any other file its absolute path. The expanded form puts each path on
its own row. Paths resolve lexically, without reading the filesystem. A leading
home directory is shown as `~`; a home directory that is empty or `/` abbreviates
nothing, so paths stay as written. The home directory is looked up only when a
full path is shown, so a listing of prompts alone needs none. When that lookup
fails, the listing reports the failure.

`[Skills]` and `[Prompts]` list the loader's skills and the session's prompt
templates. The compact form is one row of skill names, or of prompt names with a
leading `/`, trimmed and sorted for the current locale. The expanded form groups
entries under `project`, `user` and `path` (command-line and temporary resources).
Within each scope it lists plain paths first, then each package source with its
entries, all sorted for the locale. A package entry is shown relative to its
package root, or else by its path inside the npm or Git package directory. Prompts
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

The mode implements the extension UI port. A host retains it with
`ExtensionUIContext::new(mode.clone())`; the [extension UI page](extension-ui.md)
lists the operations it answers today, which are editor access, theme queries and
selection, the terminal title, tool expansion, status text and keyed widgets above
or below the editor. Dialogs, notifications and terminal input listeners keep
their own published owners.

## Typed input

`get_user_input` returns a future for the next text the user submits from the
editor, with the key bound to `tui.input.submit` (Enter by default). Submitted
text loses its outer whitespace under the shared whitespace policy: Unicode white
space other than U+0085, plus U+FEFF. U+0085, U+200B, inner spaces and line breaks
stay. Submission ignores text that is empty after trimming: no input receives it
and the history does not record it. While the session compacts or a response
streams, submitted text is queued instead (see Queued input) and no input receives
it.

Only one input waits at a time. The future starts waiting when it is first
polled, not when it is created, and it does not borrow the mode. When it waits,
the next accepted submission completes it and clears the wait. The mode keeps no
submission for a later input. A later future replaces the waiting one when it is
polled, and the replaced future stays pending.

The waiting input receives the text before the editor's history records it, so
code that runs when the input completes can already wait for the next one. Every
submission delivered as prompt text enters the history, whether or not an input
was waiting. Submission does not keep the mode alive.

Submission offers the text to the mode's commands first. Text recognized as
`/export` or `/changelog` runs that command instead: it reports on the
transcript, completes no waiting input and enters no history. Near spellings are prompt text, so
`/exporter notes.jsonl` and `/export` followed by a tab reach the input. See
[interactive commands](commands.md).

## Running the prompt loop

`run` initializes the mode, shows the startup notices, sends the startup prompts and
then keeps sending typed prompts. It never finishes on its own. It returns only an
initialization failure or a failure to show a notice.

The startup notices come in this order, before any prompt:

- a warning `Migrated credentials to auth.json: ` followed by
  `migrated_providers` joined with `", "`, when the list is not empty. Names keep
  their order, duplicates and empty names;
- an error `models.json error: ` followed by the current session's catalog load
  error, when there is one;
- `model_fallback_message` as a warning, when it is not empty. Text made only of
  white space is still shown.

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

Still to come: rendering the session's events in the transcript, queued input,
commands, rebinding the presentation to a replaced session, shutdown, update
notices and the startup check of subscription-based credentials.

## Session replay

`render_initial_messages` rebuilds the transcript from the session's current
context. Each message is shown by its existing component:

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
it with the status line `Queued message for after compaction`. The mode does not
yet send the kept messages when the compaction ends; until it does, they stay in
the pending rows, and the dequeue key restores them.

Queued messages, including those kept during a compaction, appear above the
editor. Each steering message gets a dim `Steering: <text>` row and each follow-up
message a dim `Follow-up: <text>` row, cut to one line. A last row names the keys bound to
`app.message.dequeue` (Alt+Up by default) with `to edit all queued messages`. The
keys are read when the rows are built, each part capitalized, as in `Ctrl+Alt+Q`.
With nothing queued, the rows are removed.

The dequeue key moves every queued message back into the editor: steering messages
first, then follow-up messages, separated by blank lines and followed by the current
draft when that draft is not blank. Duplicates and their order are kept. A status
line then says `Restored 1 queued message to editor`,
`Restored <n> queued messages to editor` or `No queued messages to restore`; with
nothing queued the editor is left as it was.

While a response streams, the interrupt key (Escape by default) restores the queued
messages the same way and then stops the response, even when nothing was queued. When
no response streams, the key does nothing. The queue keys do not keep the mode alive,
and a failure in them goes to the terminal runtime's error route.
## Session events

One listener carries the session's events into the mode. It is the session's own
subscription, so the removal operation the session returns is kept for the
lifecycle that ends it. Delivery is synchronous and borrowed: the mode handles
each event before the producer returns, keeps no reference to itself through the
listener, and enqueues no copy of the event. A failure goes to the terminal
runtime's error route.

An event that arrives before the presentation has started starts it first. Every
event then invalidates the footer, which the footer does not cache. The events
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
  muted working message, painted by the theme in effect when it is drawn;
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
  stops the working row and empties the status rows, removes an unfinished
  streaming row and forgets the registered calls; completed rows stay.

Rows change in place, so each frame rewrites only from its first changed row.

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

## Status lines

A status is a dim line in the transcript. When the previous status line is still
the last row, a new status replaces its text; otherwise it starts a new line.

## Shortcuts

The expand shortcut (`app.tools.expand`) flips expansion of the startup help and of
every expandable transcript row: loaded-resource sections, tool rows, shell runs,
summaries, skill rows and custom messages. The walk uses the transcript's rows as they are when it starts, so
a row a renderer appends during the walk is reached, and a row it removes is not.

The thinking shortcut (`app.thinking.toggle`) flips and saves `hideThinkingBlock`,
replays the transcript, puts a streaming row back after it with the new visibility
and shows `Thinking blocks: hidden` or `Thinking blocks: visible`. A shortcut's failure goes to the terminal runtime's error route.

## Thinking and model cycling

The thinking cycle key (`app.thinking.cycle`, Shift+Tab by default) selects the
session's next thinking level. It shows `Thinking level: <level>` as a status line
and refreshes the footer and editor border; when the model cannot reason it shows
`Current model does not support thinking` and changes nothing else. A recording
failure goes to the terminal runtime's error route.

The model cycle keys (`app.model.cycleForward` and `app.model.cycleBackward`,
Ctrl+P and Shift+Ctrl+P by default) select the next or previous scoped model, or
the next or previous available model when none are scoped. The cycle runs on the
terminal runtime, which keeps the mode until it completes. It then shows
`Switched to <name>`, using the model id when the name is empty and adding
` (thinking: <level>)` for a reasoning model whose level is not `off`, and refreshes
the footer and editor border. With no other candidate it shows
`Only one model in scope` or `Only one model available`. A session error is shown
as an error notice instead of the switch message. A failed credential lookup leaves
the selection as it was; a recording or listener failure happens after the new
model is selected, so that model stays selected.

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

The text written is the expanded text, so a collapsed paste is written in full
rather than as its marker. It goes to `maestro-editor-{milliseconds}.maestro.md` in
the temporary directory. The mode then stops the terminal and runs the editor on
the inherited terminal. A zero exit replaces the editor text with the file's text
less one final line feed; a nonzero exit, a signal or a failed launch keep the text.
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
