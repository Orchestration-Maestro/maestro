# Shared session-tree projection

`maestro_session::export_html::template` owns retained occurrence handles,
stable active-first ordering and physical/visible branch geometry. Frontends
reach it through `maestro_app::export_html::template`; the [chat selector](../chat/tree-selector.md)
keeps filter, search, fold, selection and callback state.

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
