# Browser input and timers

[`Input`](../../crates/maestro-web/src/components/input.rs) is a retained
single-line field. Construct it with owned `InputProps` and an `Rc` to the
existing `AttachmentHost`; `BrowserAttachmentHost` places its real native input
in the same canvas-relative surface registry as attachment previews.

The seven input types are text, email, password, number, URL, telephone and
search. The browser owns editing, validation, selection and autocomplete.
Small, medium (default) and large fields allocate heights of 32, 36 and 40 points.
`render` uses the width available in its parent: callers composing a field beside
a button reserve the button's width first. The wrapper receives `class_name`;
CSS classes do not replace the caller's canvas layout. Label, required marker
and error text use the supplied localization and brand presentation. Install the
page's `dark` class for dark native-control background and invalid-ring styling.

`set_props` updates the next presentation. An unchanged supplied value does not
reset native edits; a changed supplied value is assigned, as is the first value
of a recreated native element. The four optional callbacks receive actual input,
change, keydown and keyup events. `InputRef` exposes the live `HtmlInputElement`.
Drop removes the owned field and its listeners; reference replacement clears
only the assignment still naming that field. Keep reference slots unborrowed
during presentation and teardown. Use weak captures when a callback can reach
its containing field owner.

Native keydown forwards the caller's event before bridging Escape into
`AttachmentHost::take_escape`, requesting a repaint even while the canvas lacks
focus. Stopped propagation suppresses this bridge; default prevention does not.
This is a field bridge, not a dialog registration or a dialog shell.

[`Timer`](../../crates/maestro-web/src/browser/timer.rs) owns cancellation of one
native timeout or interval. `AttachmentHost::timer` takes the caller's delay and
repeat policy; explicit `cancel` and drop revoke future delivery. `spawn_local`
uses the browser's local future dispatcher. Cancelling a timer does not abort
unrelated asynchronous work already started by its callback.

Custom hosts implement the timer, local-work and Escape operations explicitly,
and exhaustive content matches handle `AttachmentContent::Input`. Native tests
exercise the public presentation and ownership seams. WebAssembly compilation
qualifies the browser types; browser interaction qualification remains tracked
in [the browser frontend issue](https://github.com/Orchestration-Maestro/maestro/issues/113).
The reusable dialog shell is separate work in
[the dialog-shell issue](https://github.com/Orchestration-Maestro/maestro/issues/1053).
