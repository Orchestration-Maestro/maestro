# Artifact panels and views

`maestro_web::tools::artifacts::create_artifact_element` constructs an owned
`Box<dyn maestro_web::ArtifactElement>` for PDF or an unrecognized filename suffix.
Other recognized specialized formats return a presentation error until their
viewers are installed; they never silently become generic downloads.

The element exposes literal filename/content getters and setters, independent
`get_header_buttons` and `render` operations, and call-scoped `ArtifactView`
localization and brand operands. Retain the element across frames. Setters request
repaint after drawing; they do not retain borrowed rendering inputs.

The generic body is selectable native text in the supplied bounded viewport:
a file glyph, literal filename and localized no-preview guidance. Body drawing
never decodes content. The existing attachment registry owns placement and native
teardown; edits keep the same published factory/root and update only changed text
nodes, preserving unrelated selection and scroll. Dropping the element removes
its own surface.

The independent header decodes the original base64 before drawing its download
control. Standard alphabet, optional valid padding, nonzero unused trailing bits
and ASCII TAB/LF/FF/CR/SPACE are admitted; malformed padding, URL-safe alphabet and
other whitespace fail. A case-sensitive `data:` prefix with a nonempty suffix
after its first `base64,` marker selects the complete suffix, including line
breaks. No text fallback is used. Reached save failures propagate without replacing
the body. MIME selection uses the last literal dot-delimited segment with Unicode
lowercase, not path normalization or a MIME database. The native byte operation
preserves the literal name and selected bytes.

`GenericArtifact` is also available through its module for direct construction.
Its initial filename and content are empty. Caller-supplied packs and localization
apply on each draw; there is no global theme or new runtime.

The [local application caller](../application/artifacts.md) can retain a supplied
presentation adapter that owns these views. This library does not install an
incomplete application presentation implementation.

## Retained panel

`maestro_web::ArtifactsPanel` owns installed views, tab layout and pending tab
scrolling. The application caller owns the only selected filename. Retain both
across frames and supply the existing host:

```rust,ignore
let panel = std::rc::Rc::new(maestro_web::ArtifactsPanel::new(host));
panel.connected_callback(&caller)?;
panel.render(ui, &caller, view)?;
```

The embedding presentation delegates its view upsert, show and removal callbacks
to this panel. The library does not install a production presentation adapter.
Tabs use record filenames in insertion order; clicking one calls the application's
`show_artifact`, not its host-open notification. Tab labels collapse HTML spaces,
tabs and line breaks into a single line; selection and downloads keep the original
filename. Horizontal padding also keeps an empty label clickable. Keyboard traversal
visits tabs, selected-view header controls, then close, with controls right-aligned.
The selected installed view
supplies the original-byte download header. A close activation calls the optional
notification without changing selection or disposing views. Header controls run
once per toolkit render pass, which can run more than once in one presentation
when layout needs another pass.

`set_collapsed` and `set_overlay` request repaint. Rendering reconciles native
visibility. Empty, collapsed and disconnected panels paint no shell; selected
header evaluation still propagates decoding errors when hidden. Reconnection
chooses the first map key only for absent or empty selection. Existing unknown
selection is retained.

`ArtifactElement::set_visibility` and `AttachmentHost::set_retained_visibility`
hide a retained root without releasing it, including before first publication.
Tab switches, collapse and disconnection preserve the view's native root,
selection and scroll. Remove a file or drop its final view owner to release its
surface. Ordinary attachment-modal closure still releases its modal-owned
children; retained panel placement does not change that ownership rule.

Inline layout reserves a left border outside the native body. Both layouts reserve
the tab-bar divider outside that body. Overlay uses viewport geometry and toolkit modal
input admission with transparent backdrop paint. **The full-viewport dimmed and
blurred backdrop is not yet composed**; [#1145](https://github.com/Orchestration-Maestro/maestro/issues/1145)
owns it. The target dims and blurs the underlying application, while the panel's
header, tabs and content stay sharp and interactive. No canvas-fill blur substitute
or partial content-only dimming is included.

## Remaining viewers

Specialized viewers remain in
[#797](https://github.com/Orchestration-Maestro/maestro/issues/797),
[#798](https://github.com/Orchestration-Maestro/maestro/issues/798),
[#816](https://github.com/Orchestration-Maestro/maestro/issues/816),
[#818](https://github.com/Orchestration-Maestro/maestro/issues/818) and
[#819](https://github.com/Orchestration-Maestro/maestro/issues/819).
HTML runtime-provider composition remains in
[#753](https://github.com/Orchestration-Maestro/maestro/issues/753).

## PDF artifacts

`tools::artifacts::pdf_artifact::PdfArtifact` retains original filename/content
and shares the overlay's owned page renderer and native error panel. Its header
uses the same complete original-byte selector as generic artifacts, with fixed
`application/pdf` MIME. Download failures propagate independently of body state.

Body drawing opens changed nonempty content and advances one page per repaint
frame. Completed native images retain their order and opaque white document
pixels; the scrollable reading wrapper adds 16-point padding. Assigning unchanged
successful content preserves page nodes, rendering progress and scroll position.
Replacing content cancels the old document and clears the prior load error;
assigning the same content after a load error retries instead. Empty content cancels
unfinished work but retains already displayed successful pages; empty content
after an error instead shows an empty body. Dropping the view removes its surface.
Errors use selectable literal native text with the localized `Error loading PDF`
heading, distinct from the overlay's `Error loading file` heading. Caller branding
styles page chrome and error text, not document pixels.
