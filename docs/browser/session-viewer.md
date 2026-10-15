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

## Header widgets

`maestro_web::export_html::template::render_header(ui, &document, &mut state, view)`
draws selectable identity, metadata, system-prompt and available-tool text using
an existing `egui::Ui`. It reads `global_stats` as prepared, independently of
`leaf_id` and later raw-data edits. Token categories retain their individual
thousand/million formatting; cost adds the four components before three-place
formatting, ignoring the supplied total.

Keep a caller-owned `HeaderState::default()` for each displayed document. It
retains prompt expansion and independent tool positions, including tools with
the same name. Reset that state when replacing the document. Long prompts start
with ten original LF-split lines; display newline normalization happens after
preview selection. Dragging selects/copies text; double clicking selects a word
without a second expansion toggle. Clicking an existing selection may clear it
and toggle immediately, following toolkit input behavior.

`HeaderView` borrows the selected `BrandPresentation` and a synchronous
`localize_date` operation. Missing or falsy timestamps display `unknown` without
calling it. On browser targets, `localize_header_date` uses the browser's native
date display, including locale/timezone and invalid-date text. Inject another
operation for an embedding host; returned date errors propagate unchanged.
Drawing scopes presentation changes without changing the outer UI style.

The returned `Vec<HeaderAction>` contains activated controls in control order:
`ToggleThinking`, `ToggleTools`, `DownloadSessionJson`. Idle frames return an
empty vector. The embedding viewer handles these requests; drawing performs no
visibility operation or download. Reached malformed text/schema data returns
`std::io::ErrorKind::InvalidData`, including invalid parameters in collapsed
tools. Parameter key order comes from `maestro_app::canonical_entries`, forwarded
from the [JSON owner](../json.md).
