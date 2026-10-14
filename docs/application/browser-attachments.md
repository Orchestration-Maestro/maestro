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
Those formats currently return `AttachmentError::Unsupported`; their extractors
are separate deliveries. Transport/body errors retain the original `FetchError`,
while rejected response status has its own `HttpStatus` variant.

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
