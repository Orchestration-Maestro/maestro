# Local model catalog

`maestro-catalog::ModelRegistry` composes the offline descriptors with an explicit
model file or an ordered list of files. `create` uses native file operations;
`with_operations` accepts
caller-selected `ModelFileOperations`. Browser callers use that adapter surface.
`in_memory` skips file access. `auth_storage` borrows the supplied credential
owner. Constructors take that credential as
`maestro_models::SharedHandle<AuthStorage>`: a shared handle on native targets
and a local one in the browser, so each caller builds the owner its target
allows. Every constructor also takes the catalog's own `ConfigValues` handle,
which runs the commands that configured credential values name; it is a platform
handle in the same way. Native file and command effects require `Send + Sync`;
browser effects may capture local `Rc` state.

`with_model_files` takes an ordered `Vec<String>` with supplied file operations.
The single-file constructors use the same path with one entry. Later files
replace supplied fields while retaining unsupplied provider and model-override
fields. Custom models overlay by provider and ID: a later definition replaces
the whole earlier definition without moving its first position. Compatibility
overlays keep the loading rules below, including one-level routing overlays.

`set_base_model_files` replaces the ordered base files below the construction-time
files and refreshes the registry, replaying dynamic providers. An empty base that
was already empty leaves the current catalog untouched. Session construction and
reload use this operation for base-package contributions.

[Configure custom models](models/custom.md) walks through editing `models.json`,
local endpoints, field defaults and compatibility settings.
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

The `modelOverrides` of a provider also apply to the models an extension registers for it,
and their `headers` are merged over the registered model's own.

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
the preceding file diagnostic before reloading. `get_error` returns an owned
`Option<String>`: an earlier observation survives clearing or replacement.

Construction loads without resetting shared account or protocol registrations.
`refresh` clears request sources and the preceding file diagnostic, resets shared
registrations, loads the underlying catalog, then replays this registry's retained
providers in live insertion order. Replacement keeps its slot; removal skips a
pending provider; removal followed by insertion moves it to the end. Callback
additions are visited during that replay. A transformation failure stops replay
with earlier effects still visible. See [OAuth transformations](models/oauth.md)
for the delegated account behavior.

## Callback re-entry

Registration, removal and refresh accept shared catalog references. The catalog
owns its private synchronization; callers need no outer lock or `RefCell` borrow.
Supplied file, credential, OAuth, configured-value and display-name effects run
without catalog, configuration or descriptor guards. Callback ownership is retired
after those guards end. Callbacks may synchronously inspect or update the same
catalog, including recursive removal and refresh.

A successful transformation publishes exactly its selected list, not a merge with
nested writes. A failing transformation leaves already-applied effects visible.
Loading publishes its local selected list only after its complete transformation
chain succeeds. Public regression witnesses in
[`catalog_reentry.rs`](../crates/maestro-catalog/tests/catalog_reentry.rs) exercise
these boundaries, live replay, owned diagnostics and native/local callers; the
[browser example](../crates/maestro-catalog/examples/browser_catalog_check.rs)
provides compile-only evidence for local browser effects.

## Dynamic providers

`register_provider(name, SharedHandle<RwLock<ProviderConfigInput>>)` validates
the incoming record, applies it, then retains it. The configuration is the same
platform handle, so registration keeps one owner from admission through replay,
display-name reads and refresh. A callback requires a nonempty provider API;
a nonempty model list requires a provider endpoint, key or OAuth, and an API on
each model or its provider. Validation failure changes nothing. Dynamic
`ProviderModelInput` values do not use the file loader's defaults or limit checks.

`ProviderConfigInput` is the [shared provider record](records.md#provider-registration-records)
with `ProviderCallbacks` beside its data: `stream_simple` and `oauth` live in its
`callbacks` field. Its JSON form deserializes straight into `ProviderConfigInput`
with no operations, so an extension's registration data needs no second record.

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
see [invocation records](records.md) for option carriers. The retained callable
supports both execution families. Forwarding preserves the original raw/simple
carrier and the caller's producer admission, without invoking its hooks. Incoming OAuth model
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

`get_api_key_for_provider(provider)` selects a credential key with
its fallback resolver disabled, then tries the configured nonempty provider key.
A present empty credential key suppresses the configured fallback. Provider-only
lookup samples that fallback after credential extraction.
`get_api_key_and_headers(model)` accepts a shared descriptor, including
one outside the catalog, and returns `ResolvedRequestAuth::Ok` or
`ResolvedRequestAuth::Error` with the winning resolution message. Both entry
points take only the model or provider and run commands through the handle the
catalog was built with.

Configured values have three forms:

- A literal string, such as `"local-key"`.
- An environment variable name, such as `"MAESTRO_PROVIDER_KEY"`.
- A shell command prefixed with `!`, such as `"!credential-helper"`.

Configured values selected for a request resolve at request time. The catalog
adds no cache: it runs the command again on every call, whether that call
succeeded or failed, and reads environment values from the process each time.
A caller that wants a different command effect, or extra caching, supplies its
own `ConfigValueOperations` handle at construction.
See [credentials](credentials.md#configured-values) for delegated value,
shell and stored-credential behavior. `clear_api_key_cache` is the callable alias
of the shared credentials command-cache clear, not a catalog cache.

Provider headers resolve before model headers even if later values replace them.
The request merges literal descriptor headers, resolved provider headers and
resolved model headers in that order, then adds `Authorization` when `authHeader`
is enabled. An enabled auth header requires a nonempty selected key. Model
request sources use both provider and model ID as their identity. Request resolution
retains its provider-source entry before credential work, but reads model identity
and model-header sources at their later consuming phases.

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

## Extension catalogs

The native component host forwards provider registration and removal through the
extension's existing runtime. Registration before binding uses its queue; direct
mutations through a retained `ModelRegistry` use that catalog instead. Catalog
refresh, authentication metadata, display names and request authentication reach
the original native owner. Invalid provider input is rejected before publication;
a rejected operation leaves independently retained credentials usable.

`ModelRegistry::auth_storage` in the guest returns the same credentials the
catalog uses. Its retained handle also keeps the catalog's credential-command
effects, available to native adapters through `config_values()`. A key changed
through that handle is visible to later catalog requests.

Guest catalogs can also be constructed over a retained credential handle from the
same host. `create(&api, auth_storage, path)` uses the supplied model path or the
application's lazy default; `in_memory(&api, auth_storage)` performs no model-file
I/O. Credential and model paths are independent. The application installs
`CatalogConstruction` on the extension runtime before loading factories, using
its configuration-path owner and credential-command effects. Catalog metadata
queries support authored credential fallback callbacks, including same-instance
synchronous reentry.
