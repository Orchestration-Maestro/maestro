# Local model catalog

`maestro-catalog::ModelRegistry` composes the offline descriptors with an explicit
model file. `create` uses native file operations; `with_operations` accepts
caller-selected `ModelFileOperations`. Browser callers use that adapter surface.
`in_memory` skips file access. `auth_storage` borrows the supplied credential owner.

## Loading and composition

The file's `providers` object supplies provider defaults, custom `models` and
built-in `modelOverrides`. Loading checks the complete document with the
[model schema checker](models/arguments.md#checking-without-conversion) before
selecting descriptor fields. Line comments and trailing commas outside strings
are removed; block comments and a leading byte-order mark are not accepted.
Configured keys and headers are not resolved during loading.

Built-in models retain their positions. Custom models replace an exact provider
and ID in its first position or append; a later duplicate replaces that slot.
Numeric-index provider keys precede other keys, which retain insertion order.
Custom API and endpoint defaults come from the provider, then its first offline
model; custom-model fields take precedence. Omitted custom metadata uses the ID
as name, text input, no reasoning, zero prices, a 128000 context window and a
16384 output limit.

Provider URL and compatibility overrides precede built-in model metadata
changes. Partial prices and thinking-level maps retain unsupplied fields;
explicit thinking nulls remain null. Compatibility overlays merge router and
gateway objects one level; nested prices and lists are replaced, not recursively
merged. Custom descriptors omit request headers.

## Observations and reload

`get_all` shares the current mutable list of shared descriptors. `find` shares
the first exact provider/ID match. Caller list and descriptor edits are visible
through those handles. `refresh` publishes a new list; retained old handles
continue to refer to the old list and descriptors.

File failures are available through `get_error`: schema failures, native parse
failures and load failures include the supplied path. A failed file replacement
removes previous custom data and uses offline descriptors as the transformation
input. Refresh clears
the preceding file diagnostic before reloading.

Stored OAuth credentials select the registered
[OAuth model transformations](models/oauth.md). Transformations run in
registration order before publication. A transformation failure propagates and
keeps the previously published list, without restoring an earlier file error.
Request authentication and dynamic registration operations are not exposed by
this module.
