# Attachment tiles and overlays

`maestro_web::AttachmentTile` renders a thumbnail or document glyph with a clipped
filename. Its public `attachment` is an `Rc<RefCell<Attachment>>`; replacing it or
editing the shared record is observed on subsequent frames. `show_delete` defaults
to false. When enabled, the remove target calls the optional `on_delete` without
opening an overlay. Hover-capable platforms reveal its paint on hover; other
platforms keep it visible. A PDF badge is not part of the image hover/click
target; the remaining image area opens the overlay. The badge background follows
the tile's rounded outline, clipped to the badge region.

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
preview/text toggle. Whitespace-only text is present. PDF previews render ordered
opaque pages through the application PDF adapter, one page per repaint frame.
The native reading wrapper scrolls, bounds overlay pages to 1000 points and keeps
completed page nodes across ordinary repaints. The bounded scrolling PDF wrapper
owns its panel background and outer border; the frame stays at the visible
viewport edges while pages scroll. The overlay viewport has no outer shadow because
native placement clips decoration to the viewport rectangle; restoring the shadow
is tracked in [#1156](https://github.com/Orchestration-Maestro/maestro/issues/1156).
Artifact padding/background remain separate.
Selecting text, replacing content or closing cancels unfinished page work.
Modern spreadsheet previews share the artifact table owner: workbook-order sheet
controls select retained native tables with formatted text, merged cells, rich
emphasis and non-executable links styled with the live accent role. Keyboard
traversal reaches and reveals offscreen sheet tabs; clipped pointer hits do not
select sheets. One sheet omits the tab strip. Each multi-sheet
body retains its own scroll offsets; ordinary repaints retain the table nodes.
The registry owns position and scaling, while the spreadsheet root keeps its
bounded height during measurement. Selecting text, replacing content or closing
releases that workbook view. Word rendering remains in
[#797](https://github.com/Orchestration-Maestro/maestro/issues/797); no extracted-text
fallback is substituted for it. Legacy spreadsheet reading remains in
[#1046](https://github.com/Orchestration-Maestro/maestro/issues/1046). A file-presentation failure replaces the body
with centered, selectable native error text while leaving download and close
available. A nonempty overlay failure stays dominant through record edits until
a mode change clears it; an empty host-presentation diagnostic stays suppressed.

## Platform ownership

The `browser::attachment_content` module exposes `AttachmentHost` and the borrowed
`AttachmentSurface`/`AttachmentContent` operands for embedding. Platform calls occur
without a widget-held attachment borrow. A host copies borrowed strings into its
retained native properties; document-preview operands carry the original shared
handle. Component identities are independent of attachment identifiers.

On WebAssembly, `BrowserAttachmentHost::new(canvas)` uses that canvas's document
and maps canvas points through its physical size and CSS bounding rectangle,
including rectangles from another same-origin realm. Native
image/pre elements retain their identity when only their rectangles move. Modal
ownership hides sibling surfaces and restores them when the overlay closes.
Surfaces provide disjoint control rectangles as `occluders`: an even-odd CSS path
cuts these out of the native content so canvas-painted controls remain visible.
The full image rectangle and fitting policy are unchanged. Native recording tests
and a CSS projection test exercise this boundary; browser hit testing, image
animation/selection and device placement remain qualified by
[#825](https://github.com/Orchestration-Maestro/maestro/issues/825).

Attachment-record downloads use the owning window's native base64 decoder, a Blob with the original
MIME type and a temporary anchor with the original filename. The object URL and
anchor are released on exit. The same byte sink serves
[session JSONL downloads](session-viewer.md#jsonl-download) through
`AttachmentHost::download_bytes`, without base64 conversion.
Download errors propagate rather than selecting a
text mode or replacing the preview with a file-display error.

`AttachmentHost::download_bytes(content, filename, mime_type)` also saves supplied
original byte slices. Controlled hosts implement this required operation; there
is no unsupported default. The browser adapter shares the same Blob, anchor and
object-URL lifecycle with attachment-record downloads. The
[generic artifact header](artifacts.md) uses this route after shared byte decoding.

Pass the selected `BrandPresentation` to each render call. Colors, type, radii and
DOM custom properties come from that presentation; image bytes are not recolored.
Production installs no default font bundle. Native tests use test-only fallback
fonts and do not qualify installed brand faces. See
[browser libraries](../libraries/browser-frontend.md) for features and licences.

Invalid caller-supplied colors propagate as `AttachmentUiError::Presentation`
through both tile and overlay drawing. Platform failures remain `Host` errors;
color admission uses the existing shared style owner, before content placement.

## Owned reading content

The same registry admits `AttachmentContent::Reading(ReadingPlacement)`.
On WebAssembly, an owned `ReadingContent` factory receives the actual canvas
`Document` and returns a `ReadingRoot` containing its native element and identified
empty control slots. It is not a
raw-markup or template interface. The host retains the complete returned owner.

`ReadingPhase::Measure` stages a hidden candidate without replacing the published
root. Placements supply actual canvas control sizes by identity.
`reading_layout(id)` reads unclipped natural height and full slot rectangles in
wrapper-local canvas points after bounded layout and native scroll clamping;
`ReadingPhase::Publish` installs complete placement. Zero-sized final viewports
are valid. Failed measurement/publication discards the candidate and retains the
previous root, native selection, scroll and placement without remounting it.
One shared transaction restores exact optional wrapper/slot style attributes and
fractional scroll positions on both axes after measurement or failed publication.
All restoration attempts run: the original operation error takes precedence,
otherwise the first cleanup error is returned. A failed native restoration write
cannot itself be guaranteed recovered. Publication applies the staged sizes and
clamped offsets before committing placement. Feature roots must contain their trailing flow spacing within
the measured root, rather than leaving an external margin outside its bounds.

The full viewport, including typography, uses the existing canvas's separate
axis scales exactly once. Native resize and scroll callbacks request repaint
through the shared event-subscription owner, with no root backedge, polling or
new timer. Observer disconnection precedes callable release. Existing modal
visibility is shared with attachments. Features paint controls at their full
reserved slot rectangles through the viewport/caller clip and interact only with
the current positive visible intersection at both pointer press and release.
Both endpoints also require an enabled control on the toolkit's input-owning layer.
A release activates only the control that owned the press. Keyboard activation
is unaffected. The header is the first mixed-content consumer.
