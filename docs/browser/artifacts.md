# Artifact panels and views

## Native bodies and headers

On browser targets, text, image, Markdown, SVG, PDF, spreadsheet, generic,
Word and HTML artifacts implement `DomContent`. Construct a format with
`default`, set its original filename/content, and mount its body through the
supplied `DomView`. Mount `get_header_buttons(&DomView)` separately. Keep both
returned roots alive while their nodes are in use, and call `update` after editing
the format's fields. Native placement needs no attachment host or retained surface.

The shared page loader registers `text-artifact`, `image-artifact`,
`markdown-artifact`, `svg-artifact`, `pdf-artifact`, `excel-artifact`,
`generic-artifact`, `docx-artifact` and `html-artifact`. Their `filename`
property reflects the filename attribute. Supply original source through the
unreflected `content` property; a content attribute does not replace it.
`artifact-console` similarly accepts its raw `logs` property.

Text and SVG source scroll long lines; Markdown source wraps. Preview/source
switches retain the existing regions, native selection and scrolling. Image errors
use the shared fallback. PDF bodies advance through cancellable native animation
frames; successful pages survive empty content and reconnection. Spreadsheet
bodies reuse the shared workbook renderer, native sheet buttons and independent
table scrolling. Neither format adds a parser, worker or overlay layer.

## Native panel

`ArtifactsPanel::default()` implements `DomContent` as an `artifacts-panel`
element. `panel.application(&view)` returns its retained
`maestro_app::browser::tools::artifacts::LocalArtifactsPanel`. Execute tools,
reconstruct history, replace runtime providers and read logs through that existing
application interface. The native adapter owns no second copy of command,
selection or replay policy.

The panel keeps one controller and element per literal filename. Commands update
that controller in place. Switching tabs, collapsing and reconnecting preserve
body roots and view state; deleting a file releases its placement. A panel's
placement key keeps another panel instance's application state separate.
`get_native_artifact_element(filename)` returns the installed
`Rc<RefCell<NativeArtifact>>`, or `None` if absent. Its public variants expose the
concrete format; `filename`, `content` and `get_header_buttons` read that same live
owner. An independently mounted header controls the panel's existing body.
Release controller borrows before executing application commands.

`set_on_open`, `set_on_artifacts_change` and `set_on_close` retain caller
notifications. `set_sandbox_url_provider` supplies the address of the next HTML
load. The panel is plain page content. Its containing screen owns responsive
placement and shared dialog/backdrop composition; setting `overlay` does not add
a second modal shell. Close notifies the caller without deleting its files.

The artifact module and crate root re-export `Artifact`, `ArtifactsParams`,
`ArtifactsPanel` and `NativeArtifact`. Concrete formats keep their documented
public paths. The application remains the owner of the artifact records and tool
parameters. Browser log readers use `Rc`;
native application log readers keep their `Arc` and thread-safety bounds.

## Retained interfaces

`maestro_web::tools::artifacts::create_artifact_element` constructs an owned
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
and `DomContent for PreviewCodeToggle` publish them as page content. The generic
file body also mounts natively through `DomContent for GenericArtifact`, and its
independent header is `GenericArtifact::get_header_buttons(view: &DomView)`. The
Word body, the HTML body and the artifact console have separate native interfaces
described below.

A mounted native body owns its `<section class="maestro-generic-artifact">` nodes,
so an update rewrites only the changed filename and guidance text. The header owns
only its controlled row and the download activation behind it: dropping the
returned root detaches that activation even while the caller keeps the node in the
document. Both roots take their nodes from `dom::DomView` and reach no host
surface, so mounting them publishes nothing and draws nothing through the retained
canvas. `dom.rs` installs the artifact header row and generic body rules with the
page stylesheet for mounted page content.

`GenericArtifact` is also available through its module for direct construction.
Its initial filename and content are empty. Caller-supplied packs and localization
apply on each draw; there is no global theme or new runtime.

The [local application caller](../application/artifacts.md) also accepts a supplied
presentation adapter for retained placement.

## Retained panel

`maestro_web::ArtifactsPanel` owns installed views, tab layout and pending tab
scrolling. The application caller owns the only selected filename. Retain both
across frames and supply the existing host:

```rust,ignore
let panel = std::rc::Rc::new(maestro_web::ArtifactsPanel::new(host));
panel.connected_callback(&caller)?;
panel.render(ui, &caller, view)?;
```

The embedding retained presentation delegates its view upsert, show and removal
callbacks to this panel.
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
input admission with transparent backdrop paint; it composes no dimmed or blurred
backdrop. The native panel gets that backdrop from the
[chat screen](components.md#chat-screen), which dims and blurs the application
behind it while the panel's header, tabs and content stay sharp and interactive.

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
have native tabs above selectable tables in direct DOM placement and toolkit tabs
in retained placement. A failed sheet in a mixed XLSX
workbook retains its tab and shows the shared selectable error panel instead of a
table; readable siblings remain available. The localized error heading updates in
place. All-corrupt workbooks retain the whole-view error. Keyboard traversal includes offscreen
tabs and reveals the focused tab; clipped pointer hits do not select a sheet.
Workbook links use the live accent role, including visited links. Table bodies retain their nodes
and independent scrolling across tab switches. The direct body bounds its table viewport. Retained placement uses the existing
registry and its control measurements.
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

## Native Word body

On browser targets, `tools::artifacts::docx_artifact::DocxArtifact` implements
`DomContent` directly. Create it with `new` or `default`, set its public `filename`
and call `set_content` with the original base64 document. Mount its body and its
independent `get_header_buttons(&DomView)` root separately. This direct interface
does not install a Word viewer in the retained panel factory.

The body reuses the document renderer with page width ignored and the other
renderer defaults retained. Pages have white backgrounds, black text and 2em
padding in both brand modes. Surrounding chrome uses the supplied brand roles.
The body and header use the same complete original-byte decoder; the header saves
with the Word document MIME type. A rejected header does not prevent mounting a
body that displays the reached diagnostic.

Call `update` after editing fields. Equal successful content and filename-only
updates preserve page nodes. Empty content keeps a successful document, but
clears a failed body. Assigning failed content again retries rendering and keeps
the diagnostic if it fails again. Each mounted root owns its document URLs and
pending shape sizing; replacement or root release drops those resources without
affecting siblings. Errors share the attachment preview's literal node builder.
The [shared Word renderer](../application/document-formats.md#word-page-rendering)
still owns document parsing and safe link output.

## Native artifact console

On browser targets, `tools::artifacts::console::Console` mounts directly through
`DomContent`. Create it with `new` or `default`, append `LogEntry` records to its
shared `logs` collection, then call `append` to publish the new lines, or `update`
after any other change.
`LogType::Log` and `LogType::Error` select the label and text role;
`BrowserSandboxValue` retains each native operand without a JSON conversion.

The console starts collapsed with following enabled. Its summary counts errors
when any are present, otherwise all lines. Expanded rows are selectable literal
text in a viewport bounded by 12rem. Arrays and other native iterables paint their
children in order; null and undefined paint no value. Ordinary objects are text,
never host templates. An active iterable cycle contributes no further text, while
a completed shared iterable contributes again at its next occurrence.

Copy uses whole-value native String conversion, not the flattened painted text.
It writes ordered `[log] value` and `[error] value` lines separated by newlines,
through the [shared copy control](transcript.md). The console builds that text
only when someone activates the control; a new line writes just its own row and
the summary count. That control owns clipboard writes and two-second icon-only
success feedback. Following scrolls every expanded update to the bottom, including
unchanged updates; disabling it preserves manual scroll. Collapse removes the
auxiliary controls without resetting the following selection.

An update may replace the log handle or use another console and localization.
Subsequent controls use those current operands, and unchanged positional rows,
selection and keyboard focus survive. Each mounted root owns its subscriptions;
releasing one does not release a sibling. The loader registers `artifact-console`,
whose unreflected `logs` property uses these same raw operands. The HTML body below
mounts the console directly.

## Native HTML artifact

On browser targets, `HtmlArtifact` mounts as an `html-artifact` element through
`DomContent`. Create it with `new` or `default`, set `filename`,
`runtime_providers` and `sandbox_url_provider`, and assign source with
`set_content`. `content` returns the source unchanged.

The body keeps two regions in place: a preview with the isolated frame and, while
there are logs, the artifact console; and a highlighted source listing. Connecting
the element creates the frame adapter, publishes it in `sandbox_iframe_ref` and
loads a nonempty source once. A changed source clears the logs and, while
connected, loads again. Equal or empty assignments, arriving logs and updates
from the same artifact never load. `execute_content` loads any source while
connected, even an empty one.
Loads read the current providers in order. A configured address replaces the
frame's address; without one the frame keeps its own. Each load appends the
completion signal after the whole source, so authored scripts, comments and
attributes stay intact. In an empty document the signal runs in the head.

Detaching the element removes the frame and its route and empties
`sandbox_iframe_ref`. Reconnecting loads again through the same adapter, and
dropping the mounted root releases it. An `update` from another artifact moves the
frame to that artifact and loads its source if it is changed and nonempty. The
`filename` attribute names the sandbox of the next load; a `content` attribute is
ignored. The unreflected `content` property assigns the body's original source.

Only messages whose `type` is `"console"` become log lines, in arrival order and
with their raw text; only the `"error"` method marks an error line. `get_logs`
returns `[log] text` and `[error] text` lines with whole-value text. With no logs it
returns the localized `No logs for {filename}` guidance, with the first placeholder
replaced by the literal filename.

`get_header_buttons(&DomView)` returns an independent row: preview/source
selection, reload, copy and download. Copy and download keep the source the header
was built with. Download saves UTF-8 bytes as `text/html` under the literal
filename: the standalone document prepared through the connected frame, otherwise
the original source. Selection and reload act on the body the artifact last
mounted or updated. Reload clears the logs and loads that body's source again.
Switching between preview and source keeps the frame, the listing, its selection
and its scroll position.
