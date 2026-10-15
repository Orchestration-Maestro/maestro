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

## Native identity

Retain a `HeaderState::new(host)` and call
`render_header(ui, &document, &mut state, HeaderView { presentation })` through
`maestro_web::export_html::template`. This fallible operation renders only
`Session: <id>` as literal browser text, with `unknown` for an absent header or
an absent or falsy ID.
It does not interpret markup, trim the ID or inspect deferred prompt/tool fields.
A truthy nonstring identity returns an invalid-data error.

The caller supplies the host and selected brand presentation. Equal identities
reuse the owned native factory; changed identities commit only after successful
measurement and placement. Dropping the state removes only its own surface.
Horizontal, vertical and wrapped parents all use an independent top-down child;
the native viewport is bounded by available height, without a second canvas
scroller or painted reading text. A transparent flow container includes the
panel's trailing spacing in that allocation without extending its background.

Metadata, help and canvas actions are tracked in
[#1090](https://github.com/Orchestration-Maestro/maestro/issues/1090), and native
prompt/tool disclosures in
[#1086](https://github.com/Orchestration-Maestro/maestro/issues/1086).
Browser Find, real selection, font reflow and observer/compositor behavior remain
qualified by [#1084](https://github.com/Orchestration-Maestro/maestro/issues/1084);
controlled hosts and WebAssembly compilation do not prove those observations.
