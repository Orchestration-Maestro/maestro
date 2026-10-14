# Session search

The `maestro-chat` library exposes session query parsing, matching and ordering
through `components::session_selector_search` and the crate root. These operations
are available independently of the session picker UI.

Queries combine ordered-character fuzzy terms with quoted contiguous phrases.
Phrases lowercase text and collapse whitespace using the shared text predicate.
An unclosed quote falls back to whitespace-separated fuzzy terms, retaining quote
characters. Only lowercase `re:` selects a case-insensitive regular expression;
expressions receive raw search text. Empty expressions report `Empty regex` in the
parsed query; invalid expressions yield no matching sessions.

Search reads the ID, name, full message text and working directory, separated by
spaces. Missing, null or non-string IDs add no ID text; their records remain
searchable through the remaining fields. Other session metadata is not searched.
Phrase and expression scores use the first match's UTF-16 offset; fuzzy scoring
is owned by the [terminal toolkit](../terminal/completion.md#shared-fuzzy-ranking).

`NameFilter::Named` independently excludes missing or blank names;
`NameFilter::default()` includes named and unnamed sessions. Blank queries preserve
name-filtered input order. For nonblank queries, `SortMode::Recent` keeps incoming
order; `Threaded` and `Relevance` rank by ascending score, then newest modification.
Equal ranks retain input order and duplicate records. Nonblank empty quotes still
reach ordering. Results borrow the original session records.

# Terminal selectors

The crate root and the corresponding `components` modules export
`ThinkingSelectorComponent`, `ThemeSelectorComponent`,
`ShowImagesSelectorComponent` and `UserMessageSelectorComponent`.
Each exposes the existing `Container` child-array editing interface. Render the
outer component, but focus its `get_select_list()` or `get_message_list()` handle.
The focus handle remains usable after clearing or dropping the outer component.

`ThinkingSelectorComponent::new` takes the current `maestro_app::ThinkingLevel`,
the available levels, select/cancel callbacks and a `LiveTheme`. Available levels
keep their supplied order and duplicates; the first current match is selected,
or the first available level when unmatched. Confirmations report typed levels,
including `Off`; an empty list cannot confirm.

`ThemeSelectorComponent::new` takes the current name, select/cancel/preview
callbacks and a `ThemeState`. Discovery errors propagate. Navigation previews
the selected theme; programmatic filtering or index changes do not preview.
The caller owns applying and restoring previews. The list displays at most ten
names, annotating only the current name.

`ShowImagesSelectorComponent::new` takes the current bool, select/cancel callbacks
and a `LiveTheme`. Its Yes/No choices report the confirmed bool.

The message module exposes `UserMessageItem`, `UserMessageList` and
`UserMessageSelectorOptions`. Pass chronological messages, select/cancel callbacks
and options holding the initial id, `LiveTheme` and `TuiRuntime` to
`UserMessageSelectorComponent::new`. An absent, empty or unmatched initial id
selects newest; duplicate matches select the first. Timestamps are accepted but
not displayed. Up/down wrap through history; confirmation reports the entry id.
The returned list's optional callbacks can be replaced or cleared, including
from inside a callback. Input uses the active configurable keybindings.

Message text replaces each LF with a space and trims the shared whitespace
predicate, then uses the terminal toolkit's cell-aware truncation. The window
shows at most ten messages, retaining absolute history positions. Borders and
list colors read the live theme; the message header/help keeps its constructor
styling. The toolkit owns list layout and text wrapping.

Only an empty message constructor schedules cancellation after 100 ms through
the supplied runtime. That one-shot keeps the original cancellation callback;
removing the panel, replacing the list callback or cancelling early does not
suppress it. The callback is released after it runs. Nonempty construction
schedules no cancellation.
