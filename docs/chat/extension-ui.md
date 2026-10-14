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
