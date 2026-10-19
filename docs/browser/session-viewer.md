# Offline session documents

`maestro_web::export_html::template::load_document(base64, query, injected_params)`
returns an owned `SessionDocument`. It reads the existing
[export payload](../export.md), without opening a history manager or
starting a provider. This reader prepares data; it does not draw a viewer.

The loader accepts standard base64 with optional complete padding and ASCII
HT/LF/FF/CR/space, then uses the application's whole-buffer UTF-8 decoder and
JSON parser. Admission failures return `std::io::ErrorKind::InvalidData`.
The `data` object retains unknown fields and raw metadata without display validation.

Injected parameters take precedence even when empty. Otherwise `query` supplies
the browser search. Form decoding selects the first `leafId` and `targetId`;
an empty leaf falls back to the raw default, while an empty target remains present.
`leaf_id` distinguishes an omitted default from null and other raw values.

## Locations and trees

`by_id` maps string identities to entry positions in `data["entries"]`.
`tool_call_map` maps assistant call identities to `(entry, content)` positions.
`label_map` maps nonempty string targets to label-record positions: a later
falsy label does not clear an earlier truthy viewer label. Replacing an identity
keeps its first key position. These indexes and `global_stats` are preparation-time
snapshots; editing the public raw data does not rebuild them.

`build_tree()` borrows the current raw entries and indexed viewer labels through
the [shared forest constructor](../tree.md#shared-construction). Its nodes cannot
outlive the document. No label timestamps are supplied by this reader.

## Statistics

`compute_stats(entries)` also works independently. Counts cover all supplied
branches, including hidden custom messages. Assistant usage accumulates the four
reported token and cost components in entry order; supplied totals are ignored,
and the reused `Usage` total slots stay zero. Models retain encounter order and
include a nonempty provider prefix when supplied.

Reached missing message receivers, malformed assistant content, truthy nonnumeric
accounting components and truthy nonstring model/provider identities return
invalid-data errors. Unused non-assistant content remains unread. Raw fields are
not narrowed to a second history schema or coerced into numeric/string values.

## JSONL download

Call [`download_session_json(&document, host)`](../../crates/maestro-web/src/export_html/template/download.rs)
through `maestro_web::export_html::template`. It downloads the raw header followed
by every stored entry in original order, independent of the selected leaf.
The header gets a default `type` of `header`; an authored `type` replaces that
value without moving its key. Missing or null headers add no record.

The payload uses compact canonical JSON, LF separators and no final newline,
with MIME type `application/x-ndjson`. A truthy string header ID becomes the
literal `<id>.jsonl` filename; an absent or falsy ID uses `session.jsonl`.
Reached unsupported metadata and serialization failures return
`std::io::Error` with their original kind and message. Supplied host failures are
wrapped by `io::Error::other` with an inspectable `AttachmentHostError` payload.

The supplied `AttachmentHost::download_bytes` consumes the prepared bytes
synchronously. The native adapter shares the
[attachment download owner](attachments.md#platform-ownership), releasing its
temporary anchor and object URL on success or a reached platform failure.

## Native header

Retain a `HeaderState::new(host)` and call
`render_header(ui, &document, &mut state, HeaderView { presentation, localize_date })`
through `maestro_web::export_html::template`. The fallible operation returns
ordered `HeaderAction` requests for thinking, tools and session download; it does
not execute them. Title, keyboard help and six metadata rows remain literal,
selectable native text. The three labelled controls remain on the canvas.

Identity uses `unknown` for an absent header or falsy ID and rejects a truthy
nonstring. Preparation consumes `global_stats` without recounting edited history
while preparing prompt/tool disclosures from the current raw fields. Models retain supplied order, token
counts use thousand/million tiers, and cost adds four components left-to-right
before three-place rounding. A falsy timestamp skips localization; otherwise the
supplied date operation receives the unchanged value. The browser's
`localize_header_date` uses native `Date` and zero-argument `toLocaleString`.

The caller supplies the host, date operation and selected brand presentation.
Equal complete preparations reuse the owned native factory; changes commit only
after successful measurement and publication. Dropping the state removes only
its own surface. No document, date-function or presentation borrow is retained.
Horizontal, vertical and wrapped parents use an independent top-down child.
Actual button dimensions size identified native slots. One live readback supplies
publication holes, full canvas paint geometry and clipped interaction rectangles.
A pointer press owns an action only inside its current positive visible slot
intersection. Release activates that same action only inside its then-current
visible intersection, including after a slot moves. Both endpoints require an
enabled control on the toolkit's input-owning layer; keyboard activation is unchanged.
The viewport is bounded by available height, including zero, without a second
canvas scroller or painted reading text. A transparent flow container includes
the panel's trailing spacing once without extending its background.

System prompts preserve literal text and blank lines. More than ten LF-split
lines produce a ten-line preview and a remaining-line hint; shorter prompts are
fully visible. Display normalizes CRLF/CR and omits NUL after counting lines.
Tools retain supplied order and literal descriptions. Parameters use canonical
key order, exact raw required-name matching, comma-joined string unions and `any`
for a falsy type. Only reached schema operations are validated, even when collapsed;
unknown raw fields remain in the document.

Prompt and parameter disclosures toggle independently on their complete native
containers. A nonempty selection in the owning window suppresses toggling.
Unchanged preparations retain the mounted root, selection and expansion; successful
replacement starts collapsed. Mounted roots own weakly captured listener state,
not factories or header state. Metadata, prompt and tools are separately decorated
siblings in one transparent native flow, with normal description whitespace and
pre-wrapped prompt text. Action execution remains
[#1044](https://github.com/Orchestration-Maestro/maestro/issues/1044); the JSONL
download operation is described above.
Actual browser tests cover slot readback, fractional scroll, selection endpoints,
bounded geometry and rollback. Browser Find, font reflow and broad
observer/compositor qualification remain
[#1084](https://github.com/Orchestration-Maestro/maestro/issues/1084) and
[#825](https://github.com/Orchestration-Maestro/maestro/issues/825); compilation or
controlled-host tests alone do not prove those observations.

## Message links

Retain `CopyLinkState::new(host)` and call
[`render_copy_link_button`](../../crates/maestro-web/src/export_html/template/copy_link/draw.rs)
through `maestro_web::export_html::template`, supplying a per-frame `CopyLinkView`.
The view borrows the selected brand presentation, active leaf identity, current
browser location, optional injected base, native slot rectangle and real
containing-message hover observation. Only activation prepares the URL. Raw leaf
and target identities remain literal through URL encoding, including empty values;
a present empty injected base differs from no base. Current-location links retain
the fragment and first nonreserved valueless query identity. Injected bases retain
their spelling, with selection appended before their fragment. Invalid current
locations return `std::io::Error` containing the URL parser cause on activation,
even with a base. Color preparation retains its native I/O error; glyph drawing
retains a typed `ControlError` payload.

The containing native message positions a 28-by-28 slot eight points from its
top/right edges and publishes the measured slot as a canvas hole. The control
registers full geometry, applies the caller clip and existing primary-gesture
admission, and returns the response for native focus reveal. It paints its own
14-point link glyph, border and selected-pack roles; message hover controls
visibility with a 150-ms `ease` transition (`cubic-bezier(0.25, 0.1, 0.25, 1)`).
Background and foreground colors use independent transitions with the same curve.
Each property retargets from its current unrounded encoded-sRGB value, with
shortened reversals; unchanged destinations keep their existing run. New controls
start at their resolved style and drop their history with their retained state.
The border switches immediately on hover or successful copy; feedback glyphs
switch immediately on success and expiry.
The control composes its background, inside border and glyph before applying
opacity once. Visibility-only animation reuses the retained image; changes to
reached raster inputs refresh it.
The copied check uses the selected pack's body typography, independent of the
parent button style. It owns no text root or viewport.

`AttachmentHost::copy_to_clipboard` starts the native call before local dispatch,
returning owned local settlement work. Its explicit `ClipboardMode` selects direct
writing or fallback. Message links select `Fallback`: the default browser host uses
its canvas document, preferring the Clipboard API and falling back to a selected
temporary textarea. Refusal or reached native failure reports false; scoped cleanup removes
the textarea even if another observer removed it first. Direct effect calls have
no control feedback. A successful control completion shows the selected pack's
passed symbol for 1500 ms. The weak completion and restoring-timer owner is
shared with the [original-text copy control](transcript.md), which retains its
independent 2000 ms policy. Later successes replace the restoring timer; failures
leave prior successful feedback unchanged. Timer registration failure restores
idle rather than publishing permanent success. Dropping the state revokes its
timer and weak UI delivery, not an already-started native copy.

Message rendering and final composition remain
[#1043](https://github.com/Orchestration-Maestro/maestro/issues/1043) and
[#1044](https://github.com/Orchestration-Maestro/maestro/issues/1044).
Permission-dependent OS clipboard and broad browser/device qualification remain
[#1084](https://github.com/Orchestration-Maestro/maestro/issues/1084) and
[#825](https://github.com/Orchestration-Maestro/maestro/issues/825).
