# Session list

`maestro-chat` exposes `SessionSelectorHeader`, `SessionList` and their options
through the crate root and `components::session_selector`. The caller supplies
session records, a live theme and host observations; these widgets do not load
or mutate sessions.

## Header

`SessionScope`, `SortMode` and `NameFilter` independently control the title and
right-hand labels. Loading progress resets whenever loading is set. Path and
rename hints are independently configurable. Confirmation takes precedence over
status, which takes precedence over ordinary help. Hints read current bindings.
The header retains three rows and clips them using terminal-cell widths.

Setters do not request rendering. A status with a caller-supplied nonzero delay
clears through the supplied runtime and requests rendering at expiry. Replacing
or clearing status cancels the prior timer; dropping the last header owner also
cancels pending work. The timer does not retain the header.

## List

Blank threaded queries build a newest-first forest, with stable ties. Other
queries use [session search](selectors.md#session-search). Named-only filtering
can make a child whose parent is excluded into a root. Each original record
remains a distinct occurrence, including canonical aliases. Parent lookup uses
the last eligible record at a canonical path. A cycle-closing parent edge is
removed so cyclic records remain visible.

Navigation clamps instead of wrapping. Up/down move one row and page actions
move ten. Confirm reports the selected authored path; cancel calls the current
cancellation slot. Ordinary input goes to the retained search input. Setters
reapply that query and clamp selection. Callback slots can be replaced or cleared,
including from inside their callbacks. Focus the list itself: its independent
flag forwards each set immediately to the search input.

Rows show names (including present empty names) or first messages, tree connectors,
counts and fixed elapsed-time units. Working directories are optional. Displayed
paths shorten a raw, nonempty home prefix rather than a path-component prefix.
C0 controls and DEL become spaces in displayed messages and metadata; message
text is additionally trimmed. Authored selection/callback identities are unchanged.
Empty views offer only actions enabled in the current key registry.

The current session's canonical identity is captured once during construction.
Visible paths resolve again during each render for active styling. Dates, home,
theme and bold policy remain live. Confirmation styling precedes active styling,
then nonempty-name styling; selection adds bold and the selected background.
The list renders at most ten session rows, with a position indicator when needed.
Invalidation is inert because these widgets have no render cache.

The application facade exposes the existing resource operations and progress
callback types for interchangeable adapters. The outer discovery controls and
asynchronous loading remain in [#808](https://github.com/Orchestration-Maestro/maestro/issues/808);
delete and rename dispatch remain in [#811](https://github.com/Orchestration-Maestro/maestro/issues/811).
