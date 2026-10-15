# Session selector

`maestro-chat` exposes `SessionSelectorComponent`, `SessionSelectorHeader`,
`SessionList` and their options through the crate root and
`components::session_selector`. The caller supplies discovery factories,
session records, a live theme and host observations; storage stays caller-owned.

## Discovery

`SessionSelectorComponent::new` takes Current/All `SessionsLoader` factories,
`SessionSelectorCallbacks` and `SessionSelectorOptions`. It starts Current
discovery before returning. Factories receive an owned `SessionListProgress`
callback and return a fallible owned `SessionsFuture`; the supplied runtime drives
completion. Only the visible scope publishes progress or results. Successful
snapshots are shared with the list; scope changes reuse caches or pending loads.
Returning to pending All keeps the loading indicator without starting another
load. Query text remains with the retained list.

Active failures display `Failed to load sessions: <message>` for 4000 ms through
the header's existing status timer. Failed All discovery can be retried by leaving
and returning to All. Automatic cancellation happens once, on successful load
completion, only while All is visible and both scopes have settled successful
empty results. Unknown or failed results do not count as empty.

Scope, sort and path controls use the live global key manager. Named filtering
uses the supplied shared manager. Dispatch order is scope, sort, named, then path,
before ordinary list input. The application action table supplies `ctrl+p`,
`ctrl+s` and `ctrl+n` defaults for path, sort and named controls. Rename help is
an explicit option; rename/delete input handling remains separate.

`get_session_list` returns the original list. A retained list keeps its discovery
callbacks useful after the outer component is released; pending futures retain
the state and list through settlement. List selection, cancellation and explicit exit
clear header status before notifying callers. Exit is a callable slot, not an
additional key binding. Toggle slots can be replaced, cleared or changed inside
their callbacks. The outer component forwards focus to the list and delegates
rendering, fallible rendering, invalidation and child-array editing to the
existing container.

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
callback types for interchangeable adapters. Delete and rename dispatch remain in [#811](https://github.com/Orchestration-Maestro/maestro/issues/811).
