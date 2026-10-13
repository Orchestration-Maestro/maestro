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
