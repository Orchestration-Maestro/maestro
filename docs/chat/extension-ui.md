# Extension UI

## Extension question dialogs

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

## Extension editor dialog

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

### External editing

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

## Shortcuts

`InteractiveMode` installs one interceptor as the editor's first input handler once the
terminal starts, over the shortcuts the session's extension runner selects from the
bindings in effect. Selection belongs to the runner: a key a reserved application action
claims is skipped with a warning, a key an ordinary application action also claims stays
with the extension and warns, and a later claim on the same key replaces the earlier
extension's callback in that key's original slot. The runner owns those messages;
[the resource listing](interactive-mode.md#loaded-resources) shows them beside the other
extension issues.

Input is matched against the selected keys by their lowercased spelling, in selection
order. The first match consumes the input and starts its callback, and every other input,
including the same key with another modifier, reaches the editor unchanged. A callback
runs on the terminal runtime after the input returns, so a handler never blocks typing,
and its failure appears as `Error: Shortcut handler error: <message>` through the
transcript's error notice.

Each callback receives a context built when the key is pressed, from the runner the
session has now. It reads the current session's working directory, model, idle state,
pending messages, context usage and system prompt, and it asks that runner to abort,
compact or shut down. A context kept from a runner the session has discarded fails as stale
instead of answering with old state. The interceptor holds the mode weakly, so it never
keeps the presentation alive; a callback that already started may keep it until it
finishes.

Rebinding replaces the whole set. An empty selection removes the interceptor, and the
retired callbacks are released without being invoked.

## Completion

`InteractiveMode` installs one completion provider on its editor when startup
succeeds. Each rebuild starts from a fresh base provider over the current session:

1. the builtin commands, in the order the workspace's command inventory lists them;
2. the loaded prompt templates, with their argument hints;
3. the extension commands, under the name each is invoked by;
4. when the `enableSkillCommands` setting is on, one `skill:<name>` command per
   loaded skill.

A description carries its provenance marker: `u`, `p` or `t` for the user, project
and temporary scopes, `u:` followed by the trimmed source for a package, and
`u:git:host/owner/repo` with `@ref` when the source names one. Source text is trimmed by the shared
whitespace scalars, which removes a byte-order mark and keeps U+0085. An absent
description yields the marker alone; a resource without provenance keeps its
description unchanged. Prompt templates and extensions keep their input order and
duplicates. An extension command whose raw name is a builtin name is skipped, so the
builtin keeps that name. Disabling skill commands rebuilds the list without them.

`/model` completes argument text against the session's scoped models, or the
catalog's available ones when none are scoped. Candidates are matched on identity
and provider together, so `opus anthropic` finds one, and are offered as
`provider/id` with the identity as the label and the provider as the description.
The callback reads the session when the request arrives, so it follows a replaced
session.

An extension command's argument callback receives exactly the typed prefix and keeps
its own failure. An absent list, an empty list and a failed callback stay distinct:
the first two offer nothing, the third reports the failure.

Typing `@` completes attachments through the search program selected during startup.
File completion keeps the candidate and applied text of the toolkit's provider. The
provider asks the search owner for the paths, so removing the selected program leaves
completion with no candidates instead of falling back to another search.
