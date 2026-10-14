# Attachment tiles and overlays

`maestro_web::AttachmentTile` renders a thumbnail or document glyph with a clipped
filename. Its public `attachment` is an `Rc<RefCell<Attachment>>`; replacing it or
editing the shared record is observed on subsequent frames. `show_delete` defaults
to false. When enabled, the remove target calls the optional `on_delete` without
opening an overlay. Hover-capable platforms reveal its paint on hover; other
platforms keep it visible. A PDF badge is not part of the image hover/click
target; the remaining image area opens the overlay.

Retain the `AttachmentOverlay` returned by a tile click, or construct one with
`AttachmentOverlay::open`. Call `render` each frame and release it when it returns
false. Escape and the close control close it; filename and full-width content
container interactions, including its side gutters, do not. Closing notifies the
optional callback before removing its surfaces.
Dropping either widget releases its surfaces, without synthesizing a close callback.
No focus capture or restoration is performed. Tab visits preview, text, download
and close in that order, omitting unavailable toggle modes.

The overlay shows original images, selectable literal text and presentation text.
Only PDF, Word and spreadsheet attachments with nonempty extracted text have a
preview/text toggle. Whitespace-only text is present. PDF, Word and spreadsheet
preview containers are delivered, but their renderers remain in
[#795](https://github.com/Orchestration-Maestro/maestro/issues/795),
[#797](https://github.com/Orchestration-Maestro/maestro/issues/797) and
[#798](https://github.com/Orchestration-Maestro/maestro/issues/798); no extracted-text
fallback is substituted for them. A file-presentation failure replaces the body
with an error panel while leaving download and close available.

## Platform ownership

The `browser::attachment_content` module exposes `AttachmentHost` and the borrowed
`AttachmentSurface`/`AttachmentContent` operands for embedding. Platform calls occur
without a widget-held attachment borrow. A host copies borrowed strings into its
retained native properties; document-preview operands carry the original shared
handle. Component identities are independent of attachment identifiers.

On WebAssembly, `BrowserAttachmentHost::new(canvas)` uses that canvas's document
and maps canvas points through its physical size and CSS bounding rectangle. Native
image/pre elements retain their identity when only their rectangles move. Modal
ownership hides sibling surfaces and restores them when the overlay closes.
Surfaces provide disjoint control rectangles as `occluders`: an even-odd CSS path
cuts these out of the native content so canvas-painted controls remain visible.
The full image rectangle and fitting policy are unchanged. Native recording tests
and a CSS projection test exercise this boundary; browser hit testing, image
animation/selection and device placement remain qualified by
[#825](https://github.com/Orchestration-Maestro/maestro/issues/825).

Downloads use the owning window's native base64 decoder, a Blob with the original
MIME type and a temporary anchor with the original filename. The object URL and
anchor are released on exit. Download errors propagate rather than selecting a
text mode or replacing the preview with a file-display error.

Pass the selected `BrandPresentation` to each render call. Colors, type, radii and
DOM custom properties come from that presentation; image bytes are not recolored.
Production installs no default font bundle. Native tests use test-only fallback
fonts and do not qualify installed brand faces. See
[browser libraries](../libraries/browser-frontend.md) for features and licences.
