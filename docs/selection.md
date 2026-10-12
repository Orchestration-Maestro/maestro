# Model selection

`maestro-catalog` resolves references against shared [model descriptors](records.md).
Selection returns a descriptor handle; it does not invoke the model or apply thinking.

## References and patterns

`find_exact_model_reference_match` tries canonical provider/ID, split provider/ID,
then bare ID. A stage with multiple matches returns absence. Exact references
trim authored whitespace, including a byte-order mark but excluding NEXT LINE.
Comparisons use Unicode lowercase.

`parse_model_pattern` tries the whole input before interpreting its last colon.
Partial ID/name matching keeps authored spacing. Aliases outrank IDs ending in
`-` plus eight ASCII digits; within either group the greatest locale-collated ID
wins, retaining the first equal candidate. Pass `model_id_comparator` a locale
reader, or supply a comparison function directly.

Thinking suffixes parse through the shared `ModelThinkingLevel` type. Invalid
suffix fallback is enabled by default: a matching prefix returns a warning
instead of thinking. Strict options disable that fallback. A valid outer suffix
replaces inner thinking unless an inner warning survives; an invalid outer suffix
replaces the inner warning. Unmatched prefixes return absence without a warning.

## Command-line input

`resolve_cli_model` borrows the existing registry through `ResolveCliModelOptions`.
Missing or empty model input returns an empty outcome; otherwise selection reads
`get_all`, without requiring configured authentication. Empty catalogs and unknown
explicit providers return catalog-owned diagnostics.

Known provider prefixes take precedence over raw gateway IDs. Explicit providers
constrain pattern matching and strip at most one matching prefix. Failed inferred
selection retries the full raw input before synthesizing a custom descriptor.
Custom fallback copies the provider's configured default, otherwise its first
model, changing only ID/name. `DEFAULT_MODEL_PER_PROVIDER` is an ordered mutable
table. Custom results are detached handles, not new registry entries.

The result keeps model, thinking, warning and error independently optional.
Selection emits no output and owns no process exit. Callers decide how to present
warnings/errors and how to apply explicit thinking choices.

## Locale reader

`model_id_comparator` reads the first present of `LC_ALL`, `LC_MESSAGES`, `LANG`.
Absent operands select `en_US`; an empty value selects ICU defaults. Encoding and
modifier suffixes are dropped, underscores become hyphens, and `C`/`POSIX` select
`en-US`. Unusable locale text selects ICU defaults. Parsed locale preferences,
including numeric, phonebook and case-first extensions, remain active.
