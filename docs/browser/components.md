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

`DialogProps` keeps portable open/close, body and literal CSS width/height operands.
On the browser target, `DialogBase::set_class_name` owns literal panel classes.
Call it before the first render or a later frame; portable `set_props` updates do
not erase the class, and closing/reopening preserves it on the same owner.
Base dimensions are `min(600px, 90vw)` by `min(600px, 80vh)`;
the standalone `dialog` helper defaults
to natural `auto` height instead. Empty dimensions remain supplied native CSS
values. Literal CSS, including intrinsic keywords, is resolved before the shared
frame transform; text and controls receive that transform once. The browser-only
`DialogHeaderProps`, `DialogContentProps`, `dialog_header` and `dialog_content`
remain at `dialogs::dialog`. `dialog_header` creates a literal heading and omits
only absent/empty descriptions; `dialog_content` composes ordered native children
with padding/gap. Their complete records keep classes with their browser readers.
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
added. Dialog operations return `dialogs::dialog::DialogUiError`, retaining native
failures inside `Host(AttachmentHostError)` and preserving leaf causes through
`Error::source`.

Full-viewport dimming/blur is intentionally absent, owned by
[the shared backdrop work](https://github.com/Orchestration-Maestro/maestro/issues/1761).
Close focus decoration is qualified through sampled raster pixels at multiple
densities; other canvas paint is qualified through visible tessellated primitives.
Broader canvas raster qualification follows [the canvas runner](https://github.com/Orchestration-Maestro/maestro/issues/1760).

## Markdown and reasoning

Markdown and the reasoning disclosure also mount as native page content and as
the `markdown-block` and `thinking-block` elements; see
[native Markdown and code](transcript.md#native-markdown-and-code) and
[page content](frontend.md#page-content).

## Native download and preview/source controls

[`download_button`](../../crates/maestro-web/src/components/download_button/dom.rs)
mounts a native button that saves the caller's original bytes through the one
scoped Blob/anchor owner. `DownloadButtonProps::new` selects a small ghost button
with a small glyph and the `text/plain` MIME type; callers then set the tooltip,
the visible label, the button size, the emphasis, the glyph extent and forced
Base64 decoding. An absent tooltip uses the localized `Download` text and an
explicitly empty one stays empty; `show_text` adds the localized label
independently. Localization is read only where it is used, so a control with an
explicit tooltip and no visible label builds even when the language cannot be
read. The button is a plain action, so it never submits a surrounding
form. Text content qualifies for decoding only at four or more characters, a
multiple of four, with the complete Base64 alphabet and at most two terminal
`=`; a failed decode saves the original text. Byte content keeps the caller's
native array, so a later write or a subview is read when the control activates.
The mounted root owns the activation listener and releases it on drop.

[`PreviewCodeToggle`](../../crates/maestro-web/src/components/preview_code_toggle.rs)
keeps one `ViewMode` in shared owned state. `mode` reads it and `set_mode` writes
it silently; `DomContent::mount` and `update` publish a native group with two icon
buttons, and `render` keeps the retained canvas pair for callers that still draw
on the canvas. A real user change updates the mode, then dispatches one bubbling
`mode-change` event whose detail is `preview` or `code`; selecting the current
mode reports nothing, and a listener that selects again keeps its own choice. The
buttons keep their native form behavior, so neither sets `type`. Registering the
`preview-code-toggle` element gives page authors the same group with an
unreflected `mode` property that reads and writes that enum and repaints the
mounted buttons. An update refreshes the buttons' localized titles and hands the
group to the controller it publishes, so later clicks reach that controller while
the buttons keep their native nodes and focus. Disconnecting
removes the group's handlers, and reconnecting restores them once with the
selection intact.

## Tool content

[Tool renderers](tool-renderers.md) provide named dispatch, retained tool headers,
input/output listings and literal console output.

## Operation errors

Input returns `components::input::InputUiError` for localization, native effects
and presentation. Shared code, Markdown and console rendering return
`components::code_block::ReadingUiError`; highlighting keeps the original theme
error, and native publication keeps `AttachmentHostError`. Copy buttons and
preview/source toggles return `components::copy_button::LocalizedControlError`
for localization, shared control drawing and selected-color I/O errors.
These Rust source interfaces keep failure kinds and causes with the operation
that owns them, without changing rendering, callback timing or retained lifetimes.

## Custom provider cards

[`CustomProviderCard`](../../crates/maestro-web/src/components/custom_provider_card.rs)
mounts through `DomContent` with the existing `CustomProvider` handle:

```rust
use std::{cell::RefCell, rc::Rc};
use maestro_web::{CustomProvider, CustomProviderCard};

fn provider_card(provider: Rc<RefCell<CustomProvider>>) -> CustomProviderCard {
    let mut card = CustomProviderCard::new(provider);
    card.on_edit = Some(Rc::new(|provider| {
        provider.borrow_mut().name = "Edited provider".into();
    }));
    card
}
```

The card starts in manual mode and displays the supplied model count. Set
`is_auto_discovery` independently of the provider protocol to display the supplied
`components::custom_provider_card::ProviderStatus`; an absent discovery result
omits that row. Names, protocol spellings and URLs remain literal native text.

Call `DomContent::update` after changing operands. Existing buttons keep their
nodes, focus and selection while their actions remain present. Activation reads
the most recently published callback and provider handle, including later edits
to that provider. It releases component borrows before calling the action, so the
action may update its containing card. Removing an action releases its binding;
disconnected roots perform no action, and reconnecting restores delivery without
a second registration. Dropping `DomRoot` releases every retained binding even
when the page still holds its nodes. Localization failures leave the last
successfully published content in place.
