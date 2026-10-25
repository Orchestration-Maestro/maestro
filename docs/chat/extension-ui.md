# Extension question dialogs

`maestro_chat::ExtensionSelectorComponent` and
`maestro_chat::ExtensionInputComponent` compose reusable questions from the
[terminal components](../terminal/components.md). Supply a title, callbacks and
options containing a live theme and the existing terminal runtime. Construction
propagates the hint owner's theme errors. Selector options retain their supplied
order and duplicates. The input accepts a placeholder but does not display it or
use it as initial text.

## Keys and callbacks

The selector reads `tui.select.up`, `tui.select.down`, `tui.select.confirm` and
`tui.select.cancel` from the current registry. Literal `k` and `j` also navigate;
navigation clamps at the endpoints without scrolling. Confirmation reports the
selected string only if it is nonempty: whitespace is not trimmed. Empty-string
options remain visible. The input reads confirm and cancel, then delegates other
keys to the toolkit's [Input](../terminal/input.md). Submission reports the exact
stored string, including empty input. Both accept LF as confirmation.

Up takes precedence over down, confirmation and cancellation in the selector;
confirmation takes precedence over cancellation in the input. Hints capture the
bindings at construction, while handling reads the live registry. Repeated
confirmation or cancellation repeats the notification. Callbacks may
synchronously render, handle input or dispose the dialog.

## Countdown and ownership

A countdown requires a supplied writer and a timeout greater than zero, in
milliseconds. There is no default deadline. The title immediately shows the
ceiling in whole seconds, updates every second and shows zero before cancellation
on expiry. Positive infinity displays `Infinity` and does not expire. NaN, zero
and negative values do not start a countdown. Title updates sample the live
accent; selector titles are bold, input titles are not. Borders sample the live
theme at render; selector option rows sample it at construction and navigation.
Prepared titles, rows and hints do not recolor merely from invalidation.

`dispose()` stops countdown work only; it does not notify or disable input.
Notifications do not implicitly dispose the countdown. The final countdown owner
cancels pending work. The countdown retains a weak writer request, not a strong
reference to the containing writer or dialog. Caller callbacks should likewise
avoid strong backreferences to their containing dialog.

Both dialogs expose their container's editable child array. Removing or replacing
chrome does not replace the retained selector list or input. Input focus keeps
its own flag and immediately forwards every setter to the retained editor,
including after the editor is removed from the displayed children. The writer
consumes the child's cursor marker to place the hardware cursor.

# Extension editor dialog

Native callers construct `maestro_chat::ExtensionEditorComponent` with a shared
`Rc<TUI>`, a live application keybinding manager, a title and
`ExtensionEditorOptions`. The options hold optional initial text, submit/cancel
callbacks, toolkit editor options, a live theme and the terminal runtime.
The dialog retains only a weak writer reference; external editing becomes inert
when that writer is gone.

The composition is a live border, spacer, prepared accent title, spacer, toolkit
multiline editor, spacer, prepared help, spacer and another live border. Title
and help capture their construction-time colors and bindings. Border colors
follow theme changes. Editor padding and completion capacity are forwarded at
construction, including the toolkit's normalization and clamping.

Global cancel bindings take precedence over the supplied application's external
editor binding, which takes precedence over normal editor input. The default
external-editor action is `app.editor.external`, bound to Ctrl+G. Submission,
newline insertion, paste handling and whitespace normalization belong to the
inner editor. Submission clears its buffer before notifying the caller;
cancellation leaves the buffer intact. Help uses the independent global
confirm/newline/cancel/external bindings and does not change after construction.

Focus setters immediately forward to the retained editor while the dialog keeps
its own focus state. The inherited container child-array interface remains
available; removing visible children does not replace the retained input target.
Callbacks may reenter rendering and input.

## External editing

The runtime's nonempty `VISUAL` wins over nonempty `EDITOR`. Selection is sampled
independently when preparing help and on each invocation. With neither configured,
the action does nothing. On Unix, commands split on literal ASCII spaces, preserving
empty arguments; quotes and tabs are not parsed, and execution is direct. Windows uses
the configured command interpreter, defaulting to `cmd.exe`, before invoking the
same inherited-terminal process bridge. The generated Windows path is passed in a
dedicated environment variable, referenced once as a quoted operand by that
interpreter; the configured command keeps its shell syntax.
No deadline is added.

The current text is written as UTF-8 to
`maestro-extension-editor-{milliseconds}.md` in the native temporary directory.
Creation is exclusive: an existing file or symlink is preserved, not overwritten
or removed. Newly created Unix files have mode 0600; Windows keeps its native ACL.
The dialog closes its write handle, stops the terminal, then launches the editor.
Only a successful zero exit replaces the text. Readback preserves a BOM and
decodes invalid UTF-8 lossily. Exactly one final LF is removed before the toolkit
normalizes line endings; no other whitespace is
trimmed until submission.

Nonzero exit, signal termination and native spawn failure preserve the buffer.
Reached filesystem, terminal and bridge failures are returned by inherent
`handle_input`; the component input trait forwards an error once through the
runtime's existing asynchronous error delivery. Cleanup removes only an owned
file and ignores removal failures. Restart is attempted after a body failure,
including file creation failure; a restart failure takes precedence. Successful
restart requests a full redraw. External editing never submits or cancels.

Interactive dialog presentation and settlement are separate from this component.

# Extension UI host in the interactive mode

`maestro_chat::InteractiveMode` implements the extension UI port, so a host
retains it with the existing `ExtensionUIContext::new(mode.clone())` and gives it
to the session's extension bindings. The bindings carry the originating event
capability, so the host only calls the bound handles it receives; it never
obtains an event bus.

This section covers the operations that exist today: dialogs, editor access,
theme queries and selection, the terminal title, tool expansion, notifications,
terminal input listeners, status text and widgets. Custom headers, footers and
editors keep their own owners and their published components.

## Dialogs

`select`, `input` and `editor` show the selector, the text input or the
multiline editor described above in place of the editor before they return.
Focus moves to the dialog and the mode requests a frame. Answering, cancelling,
a countdown expiring or the signal aborting settles the dialog once. The
countdown stops, the mode's current editor returns with focus, the mode requests
a frame and the result becomes available. A cancelled, expired or aborted dialog
answers `None`. The multiline editor takes neither a countdown nor a signal and
answers with the text its editor submits.

`confirm` shows the selector with the title and message on two lines and the
options `Yes` and `No`, in that order. Only `Yes` answers true; `No`,
cancelling, expiry and abort answer false.

An already-aborted signal answers at once: the mode shows nothing, keeps focus
and requests no frame. A countdown starts only when the timeout is greater than
zero. Aborting after the dialog settled does nothing. The shown dialog holds its
own settlement, so dropping the returned future leaves it working: a later key
still settles it and restores the editor. Settled dialogs keep neither the mode
nor its terminal alive.

`prompt_for_missing_session_cwd` asks whether to continue when a session's
stored directory no longer exists. The title is `Session cwd not found`,
followed by the session's own prompt text from
`format_missing_session_cwd_prompt`. `Yes` returns the offered directory
unchanged, even when it is empty, without checking it; `No` or cancelling
returns `None`.

## Editor access

`set_editor_text` replaces the editor text unchanged, and `get_editor_text`
returns the expanded text the editor renders. `paste_to_editor` delivers the
text as a bracketed paste, so the editor applies its own paste rules. Nothing is
trimmed and no history entry is written.

## Themes

`theme` returns the live view of the published theme, so a holder sees later
switches. `get_all_themes` lists every shipped, custom and loaded name with the
path of the first owner of that name. `get_theme` loads a name for reading and
publishes nothing; a name with no document or loaded theme is absent.

`set_theme` accepts a name or a prepared instance.

- A name is loaded and published, then watched through the effects the
  constructor was given. The name is saved only when it differs from the saved
  theme, and a frame is requested only on success. A rejected name publishes the
  fallback theme, reports the loader's own message, saves nothing and requests
  no frame.
- An instance is published as given, which stops a named theme's watch, and a
  frame is requested. The saved name is left alone.

Reading a theme never publishes one, and a failed load stays silent to the saved
name.

## Title and tool expansion

`set_title` forwards the supplied text to the terminal unchanged; there is no
formatter. `get_tools_expanded` reads the mode's own flag, and
`set_tools_expanded` applies the value to the startup help and every expandable
transcript row through the existing walk, then requests a frame.

## Notifications and terminal input

`notify` sends absent or `Info` severity to the transcript's dim status line.
Adjacent status notices reuse the last spacer and text; a warning, error or
other intervening transcript row makes the next status append a new pair.
`Warning` and `Error` use the existing labelled notice helpers. Each successful
notice requests a frame.

`on_terminal_input` adapts a bound handler to the toolkit's input dispatcher.
Registering the same handle twice invokes it once per chunk. Either remover
removes that registration; an old remover also removes a later registration
of the same handle. Dropping a remover alone leaves the registration active.
The callback may pass, replace or consume a chunk. An empty replacement reaches
later listeners but not the editor unless a later listener replaces it again.
A callback failure consumes the chunk and sends the original error to the
terminal runtime; later listeners and the editor do not receive that chunk.

The mode tracks only extension registrations for cleanup, leaving unrelated
terminal listeners intact. Cleanup is repeatable, and retained removers remain
safe to call afterwards. `stop` clears these registrations after releasing the
working loader and before disposing the footer or stopping the UI. A terminal
progress failure leaves them active; a UI-stop failure does not restore them.
Reinitializing the mode does not restore cleared callbacks. Releasing the mode
also clears its registrations.

## Failures

Both notice helpers and the status formatting keep the theme error as their own
failure, and tool expansion keeps the row's diagnostic, so a caller can
downcast the original error instead of receiving a formatted string. Prior
effects stay: an expansion sets the flag before a row's formatting can fail, and
a rejected notice adds no row.

## Status text and widgets

`set_status(key, text)` updates the footer's live status view. Replacing text
keeps its insertion position; removing and reinserting a key appends it. `None`
removes the key, while empty keys and empty text remain values. Every call
requests a frame.

`set_widget` accepts fixed text lines or a bound component factory. Widgets
appear in insertion order above the editor by default, or below it with
`WidgetPlacement::BelowEditor`. Each text widget shows at most ten supplied
strings, followed by the muted `... (widget truncated)` line when more were
supplied. Embedded newlines and styling remain the Text component's input;
the cap counts supplied strings, not their wrapped screen rows. An empty list
is a retained widget, not removal. The above container keeps one leading
spacer, including when empty; the below container has no spacer.

Replacing a widget calls its old disposer before removing it from either side,
then builds and appends the new component in the selected placement. Passing
`None` removes the key from both sides. Factories receive the live terminal
and theme through their originating callback capability. Disposer and factory
failures return the original error and keep effects already performed. A
failed disposer stops before removal; a failed factory leaves the old entry
removed. Neither failure rebuilds the last painted containers.

Headers, footers, custom editors and custom-view installation are separate
operations; these setters do not implement session replacement or reset.
