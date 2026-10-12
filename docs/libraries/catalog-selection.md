## Approved model-selection libraries

`maestro-catalog` uses `icu_collator =2.3.1` with default features, including
`compiled_data`, and `icu_locale_core =2.3.0` with defaults off and `alloc`.
Both use Unicode-3.0. The comparator accepts a caller-supplied environment reader
and exposes no ICU type. Existing `indexmap =2.14.2` with defaults and `serde`
(MIT OR Apache-2.0) holds ordered mutable provider defaults. `owo-colors =4.2.1`
(MIT, no features) styles warning lines. Internal dependencies are models,
credentials and the file-pattern leaf, whose case-insensitive matcher resolves
scope globs. See [model selection](../selection.md).
