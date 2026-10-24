# Interactive mode

`maestro_chat::InteractiveMode` holds the native terminal presentation of the
current session. This page covers what exists today: construction, startup,
the startup help, the loaded-resource listing, the transcript notices, typed
input, the export command, session replay and the expansion and thinking
shortcuts. The prompt loop,
streaming messages, startup messages, installed custom headers, the terminal
title and the other integrations are still to come; the mode reads submitted text
but does not yet prompt the session.

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
containers and the footer, in that order. It focuses the editor, binds the
transcript shortcuts, installs editor submission and starts the terminal. A
terminal start failure is returned and leaves the mode uninitialized. After a
successful start, a theme change invalidates the presentation, recolours the
editor border and requests a frame, and a branch change requests a frame. Neither
callback keeps the mode alive. Last, it lists the loaded resources in the
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
the order `[Context]`, `[Skills]`, `[Prompts]`. Each section is its heading over its
entries, followed by an empty row; `[Context]` also gets an empty row above it.
With `quietStartup` set and verbose off, nothing is listed.

`[Context]` lists the context files the session's resource loader discovered, in
discovery order. The compact form puts them on one row, separated by commas: a file
inside the working directory shows its relative path (`.` for the directory
itself), and any other file its absolute path. The expanded form puts each path on
its own row. Paths resolve lexically, without reading the filesystem. A leading
home directory is shown as `~`; an empty home directory abbreviates nothing. The home
directory is looked up only when a full path is shown, so a listing of prompts
alone needs none. When that lookup fails, the listing reports the failure.

`[Skills]` and `[Prompts]` list the loader's skills and the session's prompt
templates. The compact form is one row of skill names, or of prompt names with a
leading `/`, trimmed and sorted for the current locale. The expanded form groups
entries under `project`, `user` and `path` (command-line and temporary resources).
Within each scope it lists plain paths first, then each package source with its
entries, all sorted for the locale. A package entry is shown relative to its
package root, or else by its path inside the npm or Git package directory. Prompts
show their `/name`; when several templates share a file, the last one's name is
shown.

Sections start expanded in verbose mode or when tool output is expanded, and compact
otherwise. Each section keeps the text it was built with. Expanding or collapsing it
later repaints only its heading in the current theme.

## Notices and the editor

`show_error` and `show_warning` append an empty row and a padded notice, labelled
`Error:` or `Warning:` in the theme's error or warning colour, to the transcript.
Each requests one frame. The supplied text is kept as given, including line
breaks and its own styling. `clear_editor` empties the editor and requests a
frame; the editor's history is kept.

## Typed input

`get_user_input` returns a future for the next text the user submits from the
editor, with the key bound to `tui.input.submit` (Enter by default). Submitted
text loses its outer whitespace under the shared whitespace policy: Unicode white
space other than U+0085, plus U+FEFF. U+0085, U+200B, inner spaces and line breaks
stay. Submission ignores text that is empty after trimming: no input receives it
and the history does not record it.

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
`/export` runs that command instead: it reports on the transcript, completes no
waiting input and enters no history. Near spellings are prompt text, so
`/exporter notes.jsonl` and `/export` followed by a tab reach the input. See
[interactive commands](commands.md).

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

## Status lines

A status is a dim line in the transcript. When the previous status line is still
the last row, a new status replaces its text; otherwise it starts a new line.

## Shortcuts

The expand shortcut (`app.tools.expand`) flips expansion of the startup help and of
every expandable transcript row: loaded-resource sections, tool rows, shell runs,
summaries, skill rows and custom messages. The walk uses the transcript's rows as they are when it starts, so
a row a renderer appends during the walk is reached, and a row it removes is not.

The thinking shortcut (`app.thinking.toggle`) flips and saves `hideThinkingBlock`,
replays the transcript and shows `Thinking blocks: hidden` or `Thinking blocks:
visible`. A shortcut's failure goes to the terminal runtime's error route.

The editor border follows the session's thinking level, or the shell colour while
the editor holds a shell command.
