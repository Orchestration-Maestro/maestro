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
the action does nothing. Commands split on literal ASCII spaces, preserving empty
arguments; quotes and tabs are not parsed. Unix executes directly. Windows uses
the configured command interpreter, defaulting to `cmd.exe`, before invoking the
same inherited-terminal process bridge. No deadline is added.

The current text is written as UTF-8 to
`maestro-extension-editor-{milliseconds}.md` in the native temporary directory.
Creation is exclusive: an existing file or symlink is preserved, not overwritten
or removed. Newly created Unix files have mode 0600; Windows keeps its native ACL.
The terminal stops before launching, and only a successful zero exit replaces the
text. Readback preserves a BOM and decodes invalid UTF-8 lossily. Exactly one final
LF is removed before the toolkit normalizes line endings; no other whitespace is
trimmed until submission.

Nonzero exit, signal termination and native spawn failure preserve the buffer.
Reached filesystem, terminal and bridge failures are returned by inherent
`handle_input`; the component input trait forwards an error once through the
runtime's existing asynchronous error delivery. Cleanup removes only an owned
file and ignores removal failures. Restart is attempted after a body failure,
including file creation failure; a restart failure takes precedence. Successful
restart requests a full redraw. External editing never submits or cancels.

Interactive dialog presentation and settlement are separate from this component.
