# Attachment tiles and overlays

Tiles and overlays take the application-owned `SharedAttachment`
(`Arc<RwLock<Attachment>>`). Messages, tiles, overlays and downloads retain the
same member even after its parent list is removed, so editing the shared record is
observed on the next update. Model input and runtime globals receive independent
value snapshots; saved records remain plain attachment objects.

## Tiles

On WebAssembly, `AttachmentTile::from(attachment)` mounts through `DomContent`
without an attachment host. It shows a thumbnail or document glyph with a
whole-character truncated filename and keeps the original shared attachment,
including when its operand is replaced. A PDF badge sits outside the image's open
target; the remaining image area opens the overlay. `show_delete` defaults to
false. When enabled, the removal button calls the current `on_delete` after an
update and never opens a preview. Hover-capable pointers reveal the button on
hover, and keyboard focus reveals it too. Other pointer types keep it visible.
Opened overlays outlive their tiles.

## Native page overlay

On WebAssembly, `AttachmentOverlay::open(&view, attachment, on_close)`
opens a native preview in the `DomView` document without an `AttachmentHost`.
Creation and body-append failures return immediately. Its `DomContent` mount
exposes that same already-appended root; dropping the returned handle does not
close the connected element.

The overlay shows original images, selectable literal text and presentation text.
Only PDF, Word and spreadsheet attachments with nonempty extracted text have a
preview/text toggle; whitespace-only text counts as present. The fixed header
stays outside the scrolling body. PDF pages use the shared progressive raster
owner; Word keeps the shared renderer's fonts, page heights, headers, footers and
notes while ignoring page width. Spreadsheet tabs retain sheet nodes and their
scroll offsets; a failed sheet in a mixed workbook keeps its tab and shows the
selectable error panel while readable siblings show their tables. File/text
selection releases the previous document and clears its displayed load error. A
file-presentation failure replaces the body with centered, selectable error text
while leaving download and close available. Downloads keep the original bytes,
filename and MIME type through the existing scoped download owner.

Close, Escape and backdrop activation cancel pending page work, detach native
listeners and notify once before removing the element. Header and body clicks do
not close it. Removing the element directly releases its document resources and
listeners without synthesizing a close callback.

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

Invalid caller-supplied colors propagate as `AttachmentUiError::Presentation`.
Platform failures use `AttachmentUiError::Host(AttachmentHostError)`;
color admission uses the existing shared style owner, before content placement.

## Owned reading content

The same registry admits `AttachmentContent::Reading(ReadingPlacement)`.
On WebAssembly, an owned `ReadingContent` factory receives the actual canvas
`Document` and returns a `ReadingRoot` containing its native element and identified
empty control slots. Its optional opaque `owner` retains mounted feature state
and listener guards; passive roots use `None`. Callbacks capture this state weakly,
and the factory never retains it. Root teardown attempts detachment through the
shared subscription owner; a failed native removal may retain the callable, but
its expired weak capture cannot change released feature state. It is not a
raw-markup or template interface. The host retains the complete returned owner.

A feature whose nodes do not need the canvas can build them once and mount them
both ways. The generic file artifact constructs its glyph, filename and guidance
nodes in one builder: the retained factory wraps them in a `ReadingRoot` for the
overlay, and `DomContent` places them directly on a page. The overlay keeps
placement, measurement and clipping; the direct mount keeps only its own nodes and
reaches no host surface. Features that stay retained until their own delivery keep
using the factory alone.

An optional `ReadingRoot::commit` callback reconciles a staged update into the
previous root only after placement succeeds. This infallible callback preserves
the previous element, slots and native state; roots without it replace content
normally. Measuring a previously published factory also discards any abandoned
candidate before reading the retained root.

`ReadingPhase::Measure` stages a hidden candidate without replacing the published
root. Placements supply actual canvas control sizes by identity.
`intrinsic_width` requests a content-sized measurement bounded by the frame width.
`reading_layout(id)` reads measured width, native-resolved decoration borders,
unclipped natural height and full slot rectangles in
frame-local canvas points after bounded layout and native viewport scroll clamping;
`ReadingPhase::Publish` installs complete placement. Zero-sized final viewports
are valid. Failed measurement/publication discards the candidate and retains the
previous root, native selection, scroll and placement without remounting it.
The host retains a fixed frame, its host-created scrolling viewport and the feature
root beneath that viewport. The frame owns placement and external clipping; the
viewport explicitly clips its own border box. It is the only host-created scroller;
the frame never scrolls. Feature-owned scrollers, including individual spreadsheet
sheets, keep their own scroll state. The feature stylesheet owns decoration,
not the growing root. One shared transaction restores exact optional
frame/viewport/slot style attributes and fractional viewport scroll positions on both
axes after measurement or failed publication.
All restoration attempts run: the original operation error takes precedence,
otherwise the first cleanup error is returned. A failed native restoration write
cannot itself be guaranteed recovered. Publication applies staged sizes and placement before the infallible feature
commit. Reconciled candidates are measured with the retained viewport's offsets
under bounded prospective geometry; after the native commit, the retained viewport
receives those measured clamped offsets before controls are returned. Feature roots must contain their trailing flow spacing within
the measured root, rather than leaving an external margin outside its bounds.

The frame, including typography, uses the existing canvas's separate
axis scales exactly once. Its external clip intersects the ancestor clip with finite
drawable canvas bounds and subtracts control holes, without clipping outer decoration
to the frame's own rectangle. Zero-sized viewports remain empty. Measurement uses
bounded auto-height frame/viewport placement, without assigning height to feature roots. Native resize and scroll callbacks request repaint
through the shared event-subscription owner, with no root backedge, polling or
new timer. Observer disconnection precedes callable release. Existing modal
visibility is shared with attachments. Features paint controls at their full
reserved slot rectangles through the viewport/caller clip and interact only with
the current positive visible intersection at both pointer press and release.
Both endpoints also require an enabled control on the toolkit's input-owning layer.
A release activates only the control that owned the press. Keyboard activation
is unaffected. Natural-flow consumers can grow horizontal rows beyond their provisional height;
the offline header keeps an explicit viewport bound. The shared allocation owner
uses the same resolved bound for measurement and publication.

Retained placements keep scopes separately from widget ownership. Detaching an
ancestor scope detaches its currently placed descendants; retained widget aliases
keep resources alive but do not keep an inactive placement visible. Explicit
retained artifact visibility remains independent. A failed re-entry leaves a
detached root detached; measurement connects it only while hidden and restores
its detachment.


## Error ownership

`browser::attachment_content::AttachmentHostError` describes native effects,
presentation and reference-borrow admission. Shared glyph drawing uses
`ControlError`. Attachment operations compose these errors with localization and
local record-borrow failures. Localization and icon causes remain inspectable
through `std::error::Error::source`. Downloads and outer failures do not select a file error view. Rust callers must use the operation's error path and nested Host
payload; these source-interface changes preserve existing messages and recovery.

The independent native Word artifact also uses the error panel's node builder.
The shared builder creates literal heading/detail nodes; each caller keeps its
own failure, retry and placement policy.
