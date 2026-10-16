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
`AttachmentUiError::Presentation`; supplied host failures propagate unchanged.

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
or inspecting deferred prompt/tool fields. Models retain supplied order, token
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
visible intersection, including after a slot moves; keyboard activation is unchanged.
The viewport is bounded by available height, including zero, without a second
canvas scroller or painted reading text. A transparent flow container includes
the panel's trailing spacing once without extending its background.

Prompt/tool disclosures remain tracked in
[#1086](https://github.com/Orchestration-Maestro/maestro/issues/1086), action
execution in [#1044](https://github.com/Orchestration-Maestro/maestro/issues/1044)
and the JSONL download operation is described above.
Actual browser tests cover slot readback, fractional scroll, selection endpoints,
bounded geometry and rollback. Browser Find, font reflow and broad
observer/compositor qualification remain
[#1084](https://github.com/Orchestration-Maestro/maestro/issues/1084) and
[#825](https://github.com/Orchestration-Maestro/maestro/issues/825); compilation or
controlled-host tests alone do not prove those observations.
