# Local model catalog

`maestro-catalog::ModelRegistry` composes the offline descriptors with an explicit
model file. `create` uses native file operations; `with_operations` accepts
caller-selected `ModelFileOperations`. Browser callers use that adapter surface.
`in_memory` skips file access. `auth_storage` borrows the supplied credential owner.

[Model selection](selection.md) describes references, patterns and command-line choices.

## Errors

Construction, registration, refresh, removal and provider-key lookup return
`maestro_catalog::CatalogError`.
Its diagnostic accessor borrows the retained record; Display writes the supplied
message, including an empty message. The OAuth variant exposes the original
[account error](models/oauth.md) as its source. Authored catalog diagnostics have
no account source. File failures remain available through `get_error`, and
`ResolvedRequestAuth::Error` retains a message string.

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

Construction loads without resetting shared account or protocol registrations.
`refresh` clears request sources and the preceding file diagnostic, resets shared
registrations, loads the underlying catalog, then replays this registry's retained
providers in initial registration order. A transformation failure stops replay
with earlier effects still visible. See [OAuth transformations](models/oauth.md)
for the delegated account behavior.

## Dynamic providers

`register_provider(name, Arc<RwLock<ProviderConfigInput>>)` validates the incoming
record, applies it, then retains it. A callback requires a nonempty provider API;
a nonempty model list requires a provider endpoint, key or OAuth, and an API on
each model or its provider. Validation failure changes nothing. Dynamic
`ProviderModelInput` values do not use the file loader's defaults or limit checks.

Nonempty model lists replace this provider's descriptors and append definitions
in supplied order, including duplicates. Missing or empty lists do not replace
immediately. Endpoint/header-only updates publish replacement matching descriptors
while retaining unrelated handles; a no-op retains the list itself. Model headers
are request sources rather than descriptor metadata.

Repeat registration applies only the incoming record immediately, then merges its
defined fields into the first retained configuration handle without moving its
slot. Empty strings, lists and maps, and false flags are defined values. Provider
request sources instead replace as a whole when an incoming key is nonempty,
headers are present, or the auth-header flag is true.

OAuth registration forces the installed ID to the registration name. Raw and
simple protocol calls forward to the supplied callback under `provider:<name>`;
see [invocation records](records.md) for option carriers. Incoming OAuth model
transforms run after nonempty replacement when a modifier and stored OAuth
credentials exist. Loading and dynamic registration use the same transformation
step. Without a modifier, descriptor handles stay shared. Borrowed input elements
retain their original handles, including aliases from earlier steps. Repeated
borrows of the same object outside the input slice share one new handle within
that step. Owned descriptors receive fresh handles. Returned order and duplicates
are retained.
A transform failure leaves installed handlers, request sources and the replacement
list visible, but the failed call does not upsert the incoming configuration.
Prior edits through an already-retained configuration alias remain visible.

`unregister_provider` does nothing for unknown names. A known name is removed
before refresh restores the underlying catalog and replays surviving providers;
a replay failure does not undo removal. Retained old descriptors and callback
handles remain independent of later publication or registration changes.

## Request authentication

`get_api_key_for_provider(provider, operations)` selects a credential key with
its fallback resolver disabled, then tries the configured nonempty provider key.
A present empty credential key suppresses the configured fallback.
`get_api_key_and_headers(model, operations)` accepts a shared descriptor, including
one outside the catalog, and returns `ResolvedRequestAuth::Ok` or
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
otherwise describes a nonempty configured command, environment variable or literal key.
These metadata operations do not execute stored/configured helpers or refresh
tokens; a credential fallback callback supplied by the caller runs as given.
All three return `Result` and propagate the original credential failure before
consulting a later configured key. Availability stops at its first error without
returning a partial list; it captures the original length but reads each live slot
after the preceding callback. See [credential resolution](credentials.md#stored-credentials).
`is_using_oauth` inspects the accepted stored credential kind, not the selected key.

`get_provider_display_name` uses the retained name, retained OAuth name, current
OAuth metadata, the shared mutable
`BUILT_IN_PROVIDER_DISPLAY_NAMES` table, then the provider ID. Empty names are
present values.
