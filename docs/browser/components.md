# Browser input, timers and dialogs

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
and error text use the supplied localization and brand presentation. Each field
stacks those elements in its own column without changing caller spacing. The
native input's accessible name includes the label and localized required marker;
its accessible description contains the error. Updating or clearing these strings
updates or removes the corresponding native attributes. Focus and error rings
extend three CSS pixels outside the control at each canvas scale, bounded by the
parent clip. Install the
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

The canvas composition owns no HTML form, so fields have no implicit form
association. Validation remains available through the native input handle.
The separate dialog shell owns canvas layout, not a native form.

[`Timer`](../../crates/maestro-web/src/browser/timer.rs) owns cancellation of one
native timeout, interval or next-frame callback. `AttachmentHost::timer` takes the caller's delay and
repeat policy; explicit `cancel` and drop revoke future delivery. `spawn_local`
uses the browser's local future dispatcher. Cancelling a timer does not abort
unrelated asynchronous work already started by its callback.

Custom hosts implement the timer, local-work and Escape operations explicitly,
and exhaustive content matches handle `AttachmentContent::Input`. Native tests
exercise the public presentation and ownership seams. WebAssembly compilation
qualifies the browser types; browser interaction qualification remains tracked
in [the browser frontend issue](https://github.com/Orchestration-Maestro/maestro/issues/113).

## Dialog shells

[`DialogContext`](../../crates/maestro-web/src/dialogs/dialog_base.rs) retains the
canvas and the same native host. Retain each `DialogBase` while it is needed;
`open` eagerly captures focus and admits its modal without waiting for a render
or a result future. Call the context's `render` once per canvas frame. Its weak
registrations do not keep dropped dialogs alive. One Escape snapshot, including
the native input bridge, reaches every still-open owner even with a popup open.
A bubbling window keydown subscription also admits Escape with native predecessor
or body focus; the mounted reading root owns and releases that subscription.
Closing revokes admission and removes owned native children before notification;
drop performs the same cleanup without notification. Captured native/canvas focus
is restored on the next frame, with the existing timer owner retaining cancellation.

`DialogBody` owns an arbitrary native reading factory, control-slot dimensions
and a canvas painter. The painter receives the complete measured `ReadingLayout`
and a UI whose origin is the panel; translate relative slot rectangles by that
origin. The existing reading transaction preserves unchanged roots, selection,
editing and scrolling, and retains the old root until a replacement publishes.
Mounted child guards and Close subscriptions share the single `ReadingRoot.owner`.
Use weak captures when child callbacks can reach their containing owner.

`DialogProps` forwards literal CSS width/height and classes. Base dimensions are
`min(600px, 90vw)` by `min(600px, 80vh)`; the standalone `dialog` helper defaults
to natural `auto` height instead. Empty dimensions remain supplied native CSS
values. Literal CSS, including intrinsic keywords, is resolved before the shared
frame transform; text and controls receive that transform once. `dialog_header`
creates a literal heading and omits only absent/empty
descriptions; `dialog_content` composes ordered native children with padding/gap.
The canvas draws the panel, outer shadow and sixteen-point localized Close glyph.
One non-painting native Close button supplies keyboard/accessibility activation
and forwards native focus to the canvas focus ring. The entire Close control,
including its focus ring, has seventy-percent resting opacity and full hover
opacity; colour, opacity and focus decoration use mounted property transitions
with the 150-ms cubic-bezier(0.4, 0, 0.2, 1) curve. Interrupted transitions
continue from their current paint with shortened reversals; reopening starts
with the current resting style. Closing from a body callback also ends the current
draw's Close paint, even when the callback immediately reopens the handle.
Focus decoration retains fractional coverage in one reusable raster texture,
compositing the two disjoint annuli before control opacity. Reading text remains native,
selectable, findable and accessible; no second hidden text tree or focus trap is
added. Platform failures remain `AttachmentUiError`.

Full-viewport dimming/blur is intentionally absent, owned by
[the shared backdrop work](https://github.com/Orchestration-Maestro/maestro/issues/1145).
Close focus decoration is qualified through sampled raster pixels at multiple
densities; other canvas paint is qualified through visible tessellated primitives.
Broader canvas raster qualification follows [the canvas runner](https://github.com/Orchestration-Maestro/maestro/issues/822).
