# Approved text libraries

`maestro-text` has no dependency by default. Its `collation` feature enables
`icu_collator =2.3.1` with default features, including `compiled_data`, and
`icu_locale_core =2.3.0` with defaults off and `alloc`. Both use Unicode-3.0. The
resolved closure is 36 existing packages; no new package was added when the
direct declarations moved from the catalog, theme and toolkit crates here. The
comparator accepts a caller-supplied environment reader and exposes no ICU type.
`maestro-models` keeps `icu_collator` only as a test dependency. Test code
reads fixtures with the workspace `serde` and `serde_json`.
See [text](../text.md).
