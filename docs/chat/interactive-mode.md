# Interactive mode

`maestro_chat::InteractiveMode` holds the native terminal presentation of the
current session. This page covers what exists today: construction, startup,
the startup help, the transcript notices, typed input and the editor border. The prompt loop,
startup messages, the terminal title and the other integrations are still to come;
the mode reads submitted text but does not yet prompt the session.

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
containers and the footer, in that order. It focuses the editor, installs the editor
actions and submission and starts the terminal. A terminal start failure is returned and leaves the mode uninitialized.
After a successful start, a theme change invalidates the presentation, repaints the
editor border in the new theme and requests a frame, and a branch change requests a
frame. Neither callback keeps the mode alive.

`InteractiveModeOptions::verbose` forces the expanded startup help. Without it,
the `quietStartup` setting leaves the header empty. Otherwise the header shows the
application name and version, the compact key hints and two onboarding lines. In
verbose mode it shows the full list of key hints instead. Hints follow the loaded
bindings: several keys keep their order, and an unbound action shows no key.
Header text keeps the colours of the theme in effect when it was built.

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
accepted submission enters the history, whether or not an input was waiting.
Submission does not keep the mode alive.

## Editor border

The editor border shows the input mode. Text whose first character after leading
whitespace is `!` is a shell command, and the border takes the theme's `bashMode`
colour. Leading whitespace follows the same policy as submitted text, so U+FEFF and
a no-break space are skipped while U+0085 and U+200B are not. Otherwise the border
takes the colour of the session's thinking level, `thinkingOff` through
`thinkingXhigh`. The border is repainted and a frame requested only when an edit
switches between the two modes. A failure to read the theme is handed to the
terminal's runtime host, which reports it.
