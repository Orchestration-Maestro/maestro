# Browser attachments

The application owns attachment loading and browser-message conversion under
`maestro_app::browser`. The browser crate re-exports these entry points; DOM
file reads and rendered message components are separate features.

## Loading

`utils::attachment_utils::load_attachment` accepts a literal URL, already-read
file/blob bytes, or a bare byte buffer. Supply a
[model Fetch transport](../models/chat-completions.md#transport). URL sources issue one GET through
that port and reject non-success status before consuming the body.
Nonempty filename overrides win. Without one, file metadata supplies the name;
URL input uses the last slash-separated segment of the original full string,
with `document` for an empty segment. Blob/buffer inputs use `unnamed`.

`Attachment` stores original padded base64 bytes and byte length. Image MIME
prefixes produce image attachments with the same bytes as preview. Text MIME
prefixes or recognized text suffixes produce documents decoded by the
[text owner](../text.md). Text suffixes are case-insensitive; MIME comparisons
are case-sensitive. Existing text MIME strings, including parameters, are kept;
a suffix-only text match uses `text/plain`.

PDF, Word, PowerPoint and spreadsheet recognition precedes image/text handling.
PowerPoint recognition uses the exact presentation MIME or a case-insensitive
`.pptx` suffix. Earlier PDF/Word recognition wins; presentation recognition
precedes spreadsheet recognition. Word and spreadsheets still return
`AttachmentError::Unsupported`. Transport/body errors retain the original
`FetchError`, while rejected response status has its own `HttpStatus` variant.

PDF recognition uses exact `application/pdf` or a case-insensitive `.pdf` suffix,
selecting canonical `application/pdf`. PDF documents retain original bytes/name/size.
Each page contributes `\n<page number="N">\nTEXT\n</page>` inside the literal
`<pdf filename="NAME">` and final `\n</pdf>` wrapper. Native Unicode runs are
filtered by the shared whitespace policy without trimming retained text, then
joined with one space. Outlined text emits one run rather than separate fill/stroke
copies; invisible OCR text is retained. Page order and empty page wrappers remain.
Clip-only text and separate identical fill/stroke text-show operations await
[an interpreter text-show hook](https://github.com/Orchestration-Maestro/maestro/issues/1014).

The optional preview is standard padded base64 PNG from page one, fit to 160
pixels with truncating target dimensions, opaque white background and upscaling
for small pages. Unrepresentable targets or PNG encoding failures omit the preview
without losing extracted text. Opening failures return `AttachmentError::Pdf`,
displaying `Failed to process PDF: ` followed by the native diagnostic; this native
error has no `Error::source`. Exhaustive SDK matches must handle this variant.
The [approved PDF engine](../libraries/document-formats.md#pdf-processing) owns
text segmentation, Type3 text, bidirectional ordering and font rasterization.
Thumbnail sizing reads inherited page boxes at full precision before intersecting
the crop and media boxes and applying page rotation. Documents, caches and pixels
are scoped to the operation.

Presentation documents retain the original bytes/name/size, use the canonical
presentation MIME and have extracted text without a preview. The
[shared package reader](document-formats.md#word-package-reader) supplies selected
parts. Slides are numerically ordered with stable ties; labels are their one-based
selected positions, including omitted blank parts. Notes follow slides in numeric
order and retain the filename's digit spelling in `[Slide <digits> notes]:` headings.
The outer `<pptx filename="<name>">` wrapper retains the literal filename.

Literal nonempty `<a:t>` runs are collected without requiring valid whole-part XML.
Whitespace-only runs are filtered without trimming other runs. Accepted XML
entities are decoded once; invalid fragments retain the original complete run.
Slide runs are joined by LF inside `<slide number="<position>">`; note runs use
one space. A slide with no matched run is omitted, but a whitespace-only match
keeps an empty slide wrapper. Selected notes always produce a `<notes>` wrapper,
with headings only for retained text. Wrapper separators are LF. Package failures
return `AttachmentError::Pptx`, display `Failed to process PPTX: ` before the
package diagnostic and expose the retained package error through `Error::source`.
This additional variant requires exhaustive SDK matches to handle it.

## Messages

`components::messages` defines `UserMessageWithAttachments`, `ArtifactMessage`
and the open `CustomAgentMessages<C>` enum. Serialized records use the roles
`user-with-attachments` and `artifact`; optional fields are omitted only for
`None`, not for empty strings or lists. The records are serialization surfaces,
not persistence decoders.

`convert_attachments` retains attachment order and duplicates. Images become
image blocks; documents with nonempty extracted text become
`\n\n[Document: <name>]\n<text>` text blocks, without escaping the name or text.
Whitespace-only text is nonempty.

`default_convert_to_llm` places existing user content before attachment blocks,
filters artifacts and other custom entries, and uses the
[agent snapshot operation](../agent/api.md#message-projection) for standard
messages. The role predicates compare exact roles; caller-defined role accessors
follow the agent trait's lock contract.
