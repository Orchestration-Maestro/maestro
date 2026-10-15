# Shared session-tree projection

`maestro_session::export_html::template` owns retained occurrence handles,
stable active-first ordering, physical/visible branch geometry, content/search
reads and raw-record ancestry. Frontends
reach it through `maestro_app::export_html::template`; the [chat selector](../chat/tree-selector.md)
keeps filter, authored query, fold, selection and callback state.

`flatten_tree` borrows ordered occurrences; `retain` consumes that complete
flattening to build a flat arena. Handles keep original records and independent
raw label/timestamp slots alive after source containers are dropped. Aliases
share occurrence slots; distinct occurrences remain distinct.

`physical` receives handles from that arena. `recalculate_visual_structure`
receives the same handles, their physical rows, visible handles and the last-ID
lookup. It reconnects hidden parents to the nearest visible ancestor and retains
physical fallback rows. Parent walks are bounded by the retained identity count.

See the [session format](session_format.md) for the input records. This module
performs no filesystem, terminal or browser operations.

## Content and search

`extract_content` concatenates complete text blocks without separators or display
normalization. `extract_content_prefix` keeps the terminal's UTF-16 cutoff and
stops before reading later blocks; the uncapped display route uses `usize::MAX`.
`has_text_content` stops at the first nonblank text using the shared
[text whitespace predicate](../text.md). Reached malformed text returns
`InvalidData`; unread fields remain raw.

`get_searchable_text` returns complete lowercase viewer text. `matches_search`
applies all lowercase literal tokens to terminal previews (200 UTF-16 units),
with uncapped custom-message strings and terminal-only title, custom-state and
label-entry keywords. An empty token set succeeds without reading entry fields.
Attached labels are read from the retained occurrence's live slot. Category,
current-leaf eligibility and query/fold state remain frontend-owned.

## Paths and prefixes

`get_path` borrows root-first original records through the caller's lookup.
The viewer supplies its last-identity index. Missing targets return an empty path;
missing/empty parents stop the walk. The existing raw ancestry owner stops before
repeating an entry object, including self-links and cycles. `build_active_path_ids`
returns owned membership from the same operation, without an iteration-order promise.
This raw-record policy is separate from the retained `ancestors` iterator.

`build_tree_prefix` projects three-cell plain gutters and connectors, suppressing
virtual-root indentation. The first gutter at a level wins before its connector.
Chat replaces only a surviving connector stroke with its local folding marker;
theme and folding decisions remain in the frontend.
