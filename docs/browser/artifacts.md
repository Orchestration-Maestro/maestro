# Artifact panels and views

`maestro_web::tools::documents::create_artifact_element` constructs an owned
`Box<dyn maestro_web::ArtifactElement>` for text, images, Markdown, SVG, PDF, spreadsheets or an unrecognized filename suffix.
Other recognized specialized formats return a presentation error until their
viewers are installed; they never silently become generic downloads.

The element exposes literal filename/content getters and setters, independent
`get_header_buttons` and `render` operations, and call-scoped `ArtifactView`
localization, brand and borrowed `SyntaxHighlighter` operands. Retain the element across frames. Setters request
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

The header's download control and the preview/source selection are also
component-owned native controls: `components::download_button::dom::download_button`
and `DomContent for PreviewCodeToggle` publish them as page content. Artifact
bodies, the panel and the Chat integration still use their retained canvas
owners until their own deliveries mount those controls.

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
when layout needs another pass. The reservation never falls below the download
and close components' sizes. Cached measurements apply only to the selected
filename that produced them and are replaced when selection changes. Close keeps
its own right-edge rectangle even if a caller header overflows. Oversized caller
controls are clipped until a later layout pass measures their width. Selected-tab scrolling is repeated when that
measurement changes the available tab viewport.

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
[#797](https://github.com/Orchestration-Maestro/maestro/issues/797) and
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
pixels; the feature root adds 16-point padding inside the shared scrolling viewport. Assigning unchanged
successful content preserves page nodes, rendering progress and scroll position.
Replacing content cancels the old document and clears the prior load error;
assigning the same content after a load error retries instead. Empty content cancels
unfinished work but retains already displayed successful pages; empty content
after an error instead shows an empty body. Dropping the view removes its surface.
Panel visibility hides the retained PDF or error body without releasing it;
showing it again preserves the native root and scroll position. The retained policy
owns panel placement; leaving overlay mode does not release the PDF or error body.
Errors use selectable literal native text with the localized `Error loading PDF`
heading, distinct from the overlay's `Error loading file` heading. Caller branding
styles page chrome and error text, not document pixels.

## Spreadsheet artifacts

`tools::artifacts::excel_artifact::ExcelArtifact` shares modern workbook/table
preparation with attachment previews. Sheets keep workbook order; multiple sheets
have canvas tabs above selectable native tables. A failed sheet in a mixed XLSX
workbook retains its tab and shows the shared selectable error panel instead of a
table; readable siblings remain available. The localized error heading updates in
place. All-corrupt workbooks retain the whole-view error. Keyboard traversal includes offscreen
tabs and reveals the focused tab; clipped pointer hits do not select a sheet.
Workbook links use the live accent role, including visited links. Table bodies retain their nodes
and independent scrolling across tab switches. The native registry owns placement;
the feature root keeps bounded height while the registry measures control slots.
No spreadsheet timer or worker is installed.

The original-byte header uses the same complete decoder as other artifact headers.
The final literal dot-delimited filename segment selects `application/vnd.ms-excel`
only when Unicode lowercasing produces exactly `xls`; all other names use the
modern spreadsheet MIME. Preview failure does not disable valid-byte downloads.

Identical successful content preserves tables and sheet selection. Changed content
clears the load error and prepares replacement tables on the next repaint;
identical content after failure retries. Empty content retains a successful body,
but empty content after failure shows an empty body. Filename edits only update
metadata. Errors use the shared selectable native panel with the localized
`Error loading spreadsheet` heading and literal diagnostic or localized fallback.
Legacy XLS cells use the [shared workbook reader](../application/document-formats.md)
for formatted text, declared bounds, formula and array metadata, decoded rich-string
plain text and selected hyperlink targets. Tables retain their owned content after
parsing; sheet switches reuse those tables. The shared writer escapes binary text
without inventing run styling and applies the existing hyperlink safety policy.

## Text artifacts

`maestro_web::TextArtifact` starts with empty fields and retains one native
monospace listing. The last filename segment, lowercased with Unicode semantics,
selects the source language. Known code extensions request shared HTML
highlighting; unsupported languages and engine failures still show the complete
literal source. Other extensions render plain text. Markup remains literal unless
it is escaped output from the shared highlighter. Long lines scroll rather than
wrap. Only filename/content edits prepare the body again; unchanged redraws keep
its nodes and scroll position. Filename-only edits that leave the rendered body
unchanged also preserve native text selection.

The header retains one copy control with two-second success feedback, followed by
the original-byte download control. Copy and download both consume the original
text; even base64-looking text saves as UTF-8. The last extension selects
`image/svg+xml` for `svg`, `text/markdown` for `md`/`markdown`, otherwise `text/plain`.

## Image artifacts

`maestro_web::ImageArtifact` starts with empty fields. A case-sensitive `data:`
prefix preserves the authored source URL; other content is framed as base64 using
the filename-selected image MIME. The native image is centred and contained inside
the padded reading viewport, without a radius or disabled pointer interaction.
Its alternative text is the literal filename.

A load failure replaces the source once with an SVG labelled `Image Error` in the
selected pack's muted colour. Redraws retain that fallback; a changed source retries
the image, and pack changes recolour the fallback. Edits retain the mounted root.
Dropping either view removes only its own surface and native listeners.

The image header decodes the complete base64 payload on each draw. Data URLs with
no nonempty `base64,` payload save zero bytes. Folded payloads keep every line;
invalid base64 fails before saving and never prevents the independent body from
rendering. Image MIME follows the final extension: JPEG, GIF, WebP, SVG, BMP and
ICO have their standard types, with PNG as the default.

## Markdown and SVG artifacts

`maestro_web::MarkdownArtifact` and `maestro_web::SvgArtifact` start with empty
fields and preview selected. The factory installs Markdown for `md`/`markdown`
and SVG for `svg`, using the application's case-insensitive suffix policy.
Each retained view has its own preview/source selection; content and filename
edits preserve that selection. The localized header order is preview, code,
copy, download, with ordinary keyboard and clipped-pointer admission.
The specialized headers use 8-point gaps and a 28-point toggle group; ordinary
text headers retain their existing spacing.

Markdown preview delegates to the [shared Markdown block](transcript.md#native-markdown-and-code) with
selectable content, code-copy controls and math. Preview padding belongs to the
scrollable document inside the full-body viewport, so it scrolls away with the
content. SVG preview uses an
inert native image, not live authored markup: scripts in SVG do not execute.
The image fills the padded body with contained document pixels and native pointer
interaction. Empty SVG has no visible image; malformed SVG keeps the native image
failure rather than introducing an error panel or raster-image fallback.

Both headers copy original text and download its UTF-8 bytes, including whitespace,
Unicode and base64-looking strings. MIME is fixed to `text/markdown` or
`image/svg+xml`; filenames remain literal. Source delegates to the existing
highlighted listing: Markdown always requests Markdown and wraps long words,
while SVG always requests XML and scrolls long lines. Renaming never changes
these language or wrapping policies. The existing [copy control](transcript.md) owns
success feedback; no second timer or clipboard implementation is installed.

SVG owns one Blob URL per changed nonempty content value. Equal assignments and
idle frames keep it; replacement revokes the old URL before creating its successor,
and empty content clears it. Tab changes, collapse, modal placement and code mode
retain the URL. Real `ArtifactElement`/panel disconnection revokes it; reconnection
rebuilds it from retained text, even when code remains selected. Final owner release
removes its native surfaces and URL without affecting another view. These attachment
callbacks are distinct from retained visibility; the other views inherit no-op
callbacks.

## Error ownership

`tools::artifacts::ArtifactUiError` composes localization, shared controls, native
host effects, presentation preparation and local view-borrow failures. Native
reading factories return `browser::attachment_content::AttachmentHostError`.
Header/save failures remain independent of body-load diagnostics.

The Presentation variant carries `PresentationCause`: a literal Message or a typed
Reading or Control preparation error. `Error::source` retains the original cause
for code/Markdown rendering and localized copy or preview/source controls. Rust
callers must update error paths and nested Host/Presentation matches; messages,
retained views and recovery remain unchanged.
