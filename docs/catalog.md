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
explicit thinking nulls remain null. Compatibility overlays merge a router or
gateway slot one level only when either side is truthy (a null, false, zero or
empty-text slot with no truthy partner is kept as written); nested prices and
lists are replaced, not recursively merged. Custom descriptors omit request
headers. After schema checking, descriptor selection consumes only declared
metadata and request sources; other members are dropped without copying or decoding them. File
parsing uses the standard JSON reader recursion limit.

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
Reload clears previous request configuration before loading; an OAuth
transformation failure keeps the newly loaded request sources. Dynamic provider
registration is not exposed by this module.

## Request authentication

`get_api_key_for_provider(provider, operations)` selects a credential key with
its fallback resolver disabled, then tries the model file's configured key.
A present empty credential key suppresses the configured fallback.
`get_api_key_and_headers(model, operations)` accepts a shared descriptor, including
one outside the catalog, and returns `ResolvedRequestAuth::Success` or
`ResolvedRequestAuth::Error` with the winning resolution message.

Configured values have three forms:

- A literal string, such as `"local-key"`.
- An environment variable name, such as `"MAESTRO_PROVIDER_KEY"`.
- A shell command prefixed with `!`, such as `"!credential-helper"`.

Configured values selected for a request resolve at request time. The catalog
adds no cache: it asks the supplied operations for commands afresh on success or
failure, and reads environment values on each call. Applications
that want additional caching can supply their own `ConfigValueOperations`.
See [credentials](credentials.md#configured-values) for delegated value,
shell and stored-credential behavior. `clear_api_key_cache` is the callable alias
of the shared credentials command-cache clear, not a catalog cache.

Provider headers resolve before model headers even if later values replace them.
The request merges literal descriptor headers, resolved provider headers and
resolved model headers in that order, then adds `Authorization` when `authHeader`
is enabled. An enabled auth header requires a nonempty selected key. Model
request sources use both provider and model ID as their identity.

## Authentication metadata

`has_configured_auth` checks credential presence then configured key presence.
`get_available` returns a separate list of shared descriptors, preserving selected
slot order and duplicates. `get_provider_auth_status` prefers credential metadata,
otherwise describes a configured command, environment variable or literal key.
These metadata operations do not execute stored/configured helpers or refresh
tokens; a credential fallback callback supplied by the caller runs as given.
`is_using_oauth` inspects the accepted stored credential kind, not the selected key.

`get_provider_display_name` uses current OAuth metadata, the shared mutable
`BUILT_IN_PROVIDER_DISPLAY_NAMES` table, then the provider ID. Empty names are
present values.
