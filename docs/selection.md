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
constrain pattern matching and strip at most one matching prefix, including
provider identities containing slashes. Failed inferred selection retries the
full raw input before synthesizing a custom descriptor.
Custom fallback copies the provider's configured default, otherwise its first
model, changing only ID/name. `DEFAULT_MODEL_PER_PROVIDER` is an ordered mutable
table; the `zai` and `zai-coding-cn` providers default to `glm-5.3`. Custom results are detached handles, not new registry entries.

The result keeps model, thinking, warning and error independently optional.
`resolve_cli_model` emits no output and owns no process exit. Callers decide how to
present warnings/errors and how to apply explicit thinking choices.

## Scopes

`resolve_model_scope` resolves ordered patterns against `get_available`; which
models count as authenticated is decided by the [catalog](catalog.md).
Availability failure propagates before processing any pattern or writing warnings.
A pattern
containing `*`, `?` or `[` is a glob: a last-colon suffix is removed only when it is
a valid thinking level, and `provider/id` is tried before the bare ID through the
shared [file-pattern matcher](package-patterns.md); its failure is returned, stopping
resolution after any warnings already written. Other patterns follow
`parse_model_pattern` with its default invalid-suffix fallback.
Patterns are processed in the order supplied; within one glob, matches keep catalog
order. Duplicate provider/ID pairs are omitted; the first occurrence keeps its
thinking level. A warning from `parse_model_pattern`, when it returns one, and a
`No models match pattern` warning, when a pattern selects nothing, go to stderr,
yellow when color is requested, without stopping later patterns.

## Initial choice and restoration

`find_initial_model` considers the command line only when provider and model are
both nonempty. If `resolve_cli_model` reports an error, that error is returned as a
`CatalogError` with no result; if it selects a model, the result carries medium
thinking (`DEFAULT_THINKING_LEVEL`). Otherwise it takes the
first scoped model unless continuing, with its own scoped level, else the supplied
default, else medium thinking; then the saved default if it exists, without checking authentication,
with the supplied default thinking or medium; then the first available model matching `DEFAULT_MODEL_PER_PROVIDER` in table
order, then the first available model, then none. Every successful result carries
no fallback message. It neither restores sessions nor clamps thinking. Only the
final availability choice consumes that fallible lookup; earlier choices avoid it.

`restore_model_from_session` returns the saved model when it exists and has
configured authentication. Otherwise the supplied current model is used without
checks, then the same default-or-first available choice, then none. A selected
fallback model carries `Could not restore model <saved> (<reason>). Using <fallback>.`,
where the reason is `model no longer exists` or `no auth configured`; when no model
is selected there is no fallback message. Restoration returns a `Result`: saved-model
authentication failure precedes notices; fallback availability failure follows the
missing/unauthenticated warning. A supplied current model avoids fallback availability.

With printing requested, restoration writes success and fallback notices to stdout
(dim with color) and the failure warning to stderr (yellow with color). `ModelResolverOutput` borrows
both writers and the color choice; a failed write is ignored and never changes the
selection. A returned model is a shared handle: the catalog's own when it came from
the catalog, otherwise the caller's current model or a detached command-line descriptor.

## Locale reader

`model_id_comparator` reads the first present of `LC_ALL`, `LC_MESSAGES`, `LANG`.
Absent operands select `en_US`; an empty value selects ICU defaults. Encoding and
modifier suffixes are dropped, underscores become hyphens, and `C`/`POSIX` select
`en-US`. Unusable locale text selects ICU defaults. Parsed locale preferences,
including numeric, phonebook and case-first extensions, remain active.
