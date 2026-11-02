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

The selector checks `app.tools.expand` before every other key. Given an
`on_toggle_tools_expanded` callback, the key calls it and does nothing else: the
selection, the countdown and focus stay as they are. Without the callback the key
does nothing. Up takes precedence over down, confirmation and cancellation in the
selector; confirmation takes precedence over cancellation in the input. Hints capture the
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
compact or shut down. Its UI is the mode itself. Shutting down while idle ends the mode at
once; while a response streams, the agent's end does. A context kept from a runner the
session has discarded fails as stale
instead of answering with old state. The interceptor holds the mode weakly, so it never
keeps the presentation alive; a callback that already started may keep it until it
finishes.

Rebinding, at startup and after each session replacement, replaces the whole set.
An empty selection removes the interceptor, and the retired callbacks are released
without being invoked.

## Completion

`InteractiveMode` installs one completion provider on its editor each time it
[binds the current session](interactive-mode.md#session-binding). Each rebuild starts
from a fresh base provider over the current session:

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

`add_autocomplete_provider` retains the supplied bound factory and rebuilds completion
before returning. Each rebuild folds all registered factories over a fresh base in
registration order; the last wrapper receives requests first. Registering the same
handle twice keeps both registrations. A factory may append another registration
while rebuilding, and that rebuild sees the append. Factory calls retain their
originating extension's event capability even when another host receives the handle.

Only a successful rebuild replaces the retained provider and the default editor's
provider, with the same handle. When a custom editor is current, it receives that
handle too, once per rebuild. A failing factory returns its original error, skips
later factories and leaves the installed provider unchanged. Its registration remains
for later rebuilds. Settings changes that rebuild completion report failures through
the terminal runtime after saving the setting.

Typing `@` completes attachments through the search program selected during startup.
File completion keeps the candidate and applied text of the toolkit's provider. The
provider asks the search owner for the paths, so removing the selected program leaves
completion with no candidates instead of falling back to another search.

# Extension UI host in the interactive mode

`maestro_chat::InteractiveMode` implements the extension UI port. Its `init`
gives the mode to the session's extension bindings through
`ExtensionUIContextBinding::borrowed`, because the mode already owns the session.
A UI context an extension obtains keeps the mode alive while it is held; once the
mode is released, the session's contexts answer with the headless results. Another
host retains it with the existing `ExtensionUIContext::new(mode.clone())`. The
bindings carry the originating event
capability, so the host only calls the bound handles it receives; it never
obtains an event bus.

This section covers the operations that exist today: dialogs, editor access,
theme queries and selection, the terminal title, tool expansion, notifications,
terminal input listeners, completion wrappers, status text and widgets. Custom headers,
footers and editors keep their own owners and their published components.

## Dialogs

`select`, `input` and `editor` push the selector, the text input or the
multiline editor described above on the dialog stack before they return. The
newest dialog is shown in place of the editor with focus, and the mode requests
a frame. A dialog it covers stays open and keeps its state. Answering,
cancelling, a countdown expiring or the signal aborting settles the dialog once,
whether it is shown or covered. The countdown stops and the dialog leaves the
stack. When it was on top, the dialog below it, or the mode's current editor once
none is left, returns with focus and the mode requests a frame; a covered dialog
leaves the screen as it is. The result then becomes available. A cancelled, expired or aborted dialog
answers `None`. The multiline editor takes neither a countdown nor a signal and
answers with the text its editor submits.

`confirm` shows the selector with the title and message on two lines and the
options `Yes` and `No`, in that order. In the selectors `select` and `confirm`
show, and in the session-directory, branch-summary and import prompts shown the
same way, the tool-expand key flips the expansion of tool output and summaries as
the editor's key does. Only `Yes` answers true; `No`,
cancelling, expiry and abort answer false.

An already-aborted signal answers at once: the mode shows nothing, keeps focus
and requests no frame. A countdown starts only when the timeout is greater than
zero. Aborting after the dialog settled does nothing. The shown dialog holds its
own settlement, so dropping the returned future leaves it working: a later key
still settles it and takes it off the stack. Settled dialogs keep neither the mode
nor its terminal alive.

`prompt_for_missing_session_cwd` asks whether to continue when a session's
stored directory no longer exists. The title is `Session cwd not found`,
followed by the session's own prompt text from
`format_missing_session_cwd_prompt`. `Yes` returns the offered directory
unchanged, even when it is empty, without checking it; `No` or cancelling
returns `None`.

## Editor access

`set_editor_text` hands the text to the current editor unchanged, and the editor
stores it with its own newline and tab normalization. `get_editor_text` returns
the editor's expanded text, or its raw text when it does not expand.
`paste_to_editor` delivers the text to the current editor as a bracketed paste,
so the editor applies its own paste rules. Nothing is trimmed and no history
entry is written. An editor's own failure is returned unchanged.

## Custom editors

`set_editor_component` keeps the factory, which `get_editor_component` returns,
then replaces the current editor with the one the factory builds from the live
terminal, the editor theme and the application keybindings. `None` restores the
default editor. The current raw text moves to the new editor and the new editor
takes focus.

The new editor receives the default editor's submit and change callbacks, its
border painter when the new editor has a border, its padding and the completion
provider the mode last installed. An editor that implements the application
capability also receives every application action in its order, in the map it
already shares. Its own interrupt, empty-editor exit, image-paste and shortcut
callbacks stay; each missing one calls the default editor's callback current at
that moment. Optional operations the new editor lacks are skipped.

The mode's own text, history, insertion, completion, border, padding and
completion-height operations reach the current editor. Application actions and
callbacks stay on the default editor. Settings, the settings panel and completion
rebuilds update the default editor first and then a distinct current editor once;
when the default editor is current it is updated once. If a callable replaces the
editor, the next settings operation reaches the newly current editor.

A failure stops at its own phase and returns, or reports, the original error.
The factory stays set. A failing factory or new editor leaves the editor
container empty and the previous editor current; a failing read of the current
text changes nothing else.

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
`set_tools_expanded` applies the value to the active header and every expandable
transcript row through the existing walk, then requests a frame. The active header
is the extension header when one is installed, which follows the value only when it
supplies `set_expanded`; otherwise it is the built-in startup help.

## Headers and footers

`set_header` and `set_footer` replace the built-in header and footer with a
component an extension builds, and `None` restores the built-in one. Each call
first calls the current extension component's disposer, then requests a frame
once the replacement is in place.

A header factory receives the live terminal and theme. The new header takes the
active header's place in the header container; when that header is no longer
there, a new extension header goes first and restoring the built-in one adds
nothing. An extension header that supplies `set_expanded` receives the current
tool expansion as soon as it is built, and the built-in header receives it when
restored. Before the mode builds its own header, `set_header` does nothing and
calls no factory.

A footer factory also receives the live footer metadata, so a footer reads later
status, branch and provider-count changes. The current footer is removed, and the
new one is appended after construction.

A failing disposer stops before anything is removed and leaves the current
component in place. A failing factory leaves the old component disposed: the old
header stays shown, and the old footer stays removed with no footer appended.
Either failure returns the original error and requests no frame.

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

## Session replacement

Before each session replacement invalidates the current session, the mode resets
what extensions changed, in this order:

1. The dialog stack empties: every dialog answers `None`, as its own
   cancellation does, which stops its countdown and signal watch, and the editor
   returns when the stack held anything.
2. Every custom view that has not settled ends with no value: its own overlay is
   removed and its component disposed once. A view still building disposes its
   component when it arrives, without showing it. The editor is left to step 5.
3. The extension terminal input listeners are removed.
4. The built-in footer and header return, the widgets are disposed and removed,
   and the status texts are cleared.
5. The default editor returns with the current editor's raw text and no
   extension shortcuts. The completion wrappers are dropped and completion is
   rebuilt over a fresh base provider.
6. The title is rewritten, and the working message, visibility, indicator and
   hidden-thinking label return to their defaults. A working row that is already
   running shows the default message with the interrupt key.

The mode installs this reset on the runtime when it is constructed, reaching the
mode weakly. A failing step returns its original error, keeps the steps before it,
and stops the replacement before the session is disposed. Clearing the widgets
calls each disposer while walking the live maps, so widgets a disposer adds are
disposed too and widgets it removes are skipped; a disposer failure stops before
either map is emptied.

The stack retains each dialog's component until settlement, so dialogs opened
concurrently, of any kind, wait in order. A dialog opened from inside a custom
view goes on top, and settling it shows the custom view again.

Working messages, visibility, indicators and hidden-thinking labels an extension
sets stay in effect across prompt turns until the extension changes them or the
session is replaced.

## Custom views

`custom` shows the component an extension builds, at once or later, on the
dialog stack in place of the editor with focus, or as an overlay when `overlay`
is `Some(true)`. Overlays are not on the stack. The
factory receives the live terminal, theme, application keybindings and the
view's completion. The current editor's raw text is kept when the view starts.

An overlay uses the supplied static options, keeping the same shared options so
later edits reach the layout, or calls the resolver once when the view is shown.
Without options, a nonzero component `width` becomes the overlay width in whole
cells, at least one, as image widths count them; an empty options value still
wins over the component width. `on_handle` receives the overlay's handle after
it is shown.

The first `done` closes the view: it removes the view's own overlay, or takes the
inline view off the stack, which shows the entry below it or the editor, and puts
the kept text back in the current editor. It then settles the result and disposes
the component. A disposal failure is ignored once the view has closed.
Later calls do not close, restore or dispose again. When putting the editor back
fails, that original failure is the view's result and every later `done` returns
it again; no value is committed.

A view completed before its component exists never shows that component and
disposes it once. Completing while the overlay options resolve shows nothing.
Completing in an admission visibility callback removes that view's overlay
before setup returns; `on_handle` is not called.
A failing factory, resolver or handle callback settles the view with the
original error: an inline view leaves the stack and puts the kept text back, and
an overlay already shown
is removed and its component disposed. An overlay cleanup error does not replace
the initiating failure. A terminal failure during admission removes the overlay
and disposes its component. When putting the editor back fails too,
that failure is the result. Closing one overlay leaves every other view shown,
and focus returns to the overlay still shown.

The mode keeps each view and its building work until the view settles, so
dropping the returned future neither cancels the build nor closes the view.
A settled view releases its setup callbacks, component, overlay handle and the
mode's registration.
