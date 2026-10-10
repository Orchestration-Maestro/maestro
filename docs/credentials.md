# Credentials

`maestro-credentials` keeps stored credentials, resolves credential values and
ordered headers, selects request keys, logs in to OAuth providers, refreshes
expired OAuth tokens and formats login guidance.

## Stored credentials

`AuthStorage` accepts the stored records when it is created, on `reload` and when a
locked refresh rereads the document, adopting a newer one; other reads and changes use
that accepted snapshot. Missing or empty
stored text is an empty store. Text that is not a JSON object, or cannot be read,
locked or parsed, is a load failure: the accepted records are kept, the failure
is recorded for `drain_errors` (oldest first), and `set`/`remove` change only
accepted memory until a `reload` or a locked refresh reread succeeds. Construction never fails.

`set` and `remove` change accepted memory first, then reread the stored text under
the backend's exclusion and change only that provider in it; records other writers
added stay in the file and are not imported into memory. A persistence failure is
recorded and nothing is rolled back. `logout` removes the stored record only.

Text this owner writes is two-space JSON without a trailing newline; reloading
existing text never rewrites it. Accepted records keep their order: a replaced
record keeps its position, a new one is appended, and `list` and `get_all` follow
it. A `set` or `remove` rewrite keeps the order of the freshly reread file, which another
writer may have changed; a refresh accepts the order of the file it rereads. `get` and `get_all` return
typed copies of complete `api_key` and `oauth` objects only; `list` and `has` cover
every stored record, whatever its shape, and rewrites keep records that do not
decode. Expiry times of typed credentials you supply are written without a fraction when
whole; preserved stored records keep their numeric values, written in the JSON serializer's spelling (for example `1e3` becomes `1000.0`), not their original spelling.

`has_auth` is true for a runtime override (even empty), a stored record, a usable
provider environment value or a nonempty fallback key. `get_auth_status` reports
the first of stored, runtime (`--api-key`), environment (first populated variable
name) and fallback (`custom provider config`), without key values, stored helper
resolution or refresh. The fallback resolver you supply runs as given. See the [request authentication documentation](request-authentication.md) for
environment discovery.

## Request keys

`get_api_key(provider, include_fallback, operations)` tries a nonempty runtime
override, the stored record, the provider environment value and, unless
`include_fallback` is false, the fallback resolver, stopping at the first that
applies. The stored record decides as follows:

- A complete `api_key` record resolves through the cached configured-value
  resolver below; `None` and the empty string both end the lookup.
- A malformed `api_key` or `oauth` record, or an `oauth` record of an unregistered
  provider, ends the lookup without a key. A record of any other type, or one that is
  not an object, is ignored and the lookup continues.
- An `oauth` record with `now < expires`, read from the `maestro_models::timestamp_now` clock,
  yields the provider's extracted key; an extraction failure is returned to the
  caller and is not recorded.
- Otherwise the token is refreshed, as described next.

The future is `Send` on native targets and does not borrow `operations`, because the
runtime override and stored key are resolved before it is built.

### Refresh and login

An expired token is refreshed through the backend's `with_lock_async` (the file adapter
holds its sidecar lock meanwhile; the memory adapter holds nothing, see below). The stored
text is reread and accepted, publishing it and clearing a recorded load failure.
A record that is then absent, no longer `oauth` or no longer complete gives an empty
result, and the lookup continues with the environment and the fallback resolver as
they are at that moment. A token with `now < expires` is reused. Otherwise the
model library's [`get_oauth_api_key`](models/oauth.md), which checks expiry itself,
refreshes it; the result replaces that provider's record in the
accepted records as they are after the refresh (so edits and reloads of other providers' records made while the
refresh was pending are kept), is published, and the whole accepted document is then
written.

A lock, read, parse, provider, key-extraction or write failure is recorded for
`drain_errors`, and the store is reloaded. Only a stored `oauth` record that is complete
and has `now < expires` then supplies a key; otherwise the result is `None` with no
environment or fallback lookup. A failed reload records its own error and keeps the
accepted records, so a token published before a failed write still counts. Refresh
credentials that were not replaced stay stored for the next request.

Dropping a pending request releases a lock it holds and never starts or resumes its
callback; effects already completed, including published accepted records, are not undone.

`login(provider, callbacks)` runs the currently registered provider and stores the
credentials it returns as an `oauth` record, persisting like `set`; an unregistered
provider fails with `Unknown OAuth provider: <id>`. `get_oauth_providers` returns the
model library's registered providers in registration order, sharing their
implementations; see the [OAuth documentation](models/oauth.md) for the provider
interface.

## File and memory backends

`FileAuthStorageBackend` (native builds) keeps credentials at a caller-supplied
path, relative paths resolving against the working directory. Missing parent
directories are created with mode 0700 and a missing file is created, only while
the sidecar `<path>.lock` is held, containing `{}` with mode 0600 on Unix; writes
set mode 0600. The lock is tried up to ten times, waiting 20 ms only between contended attempts; other lock errors return
at once, and the lock is released after every outcome. File bytes are read as
lossy UTF-8. The asynchronous operation `with_lock_async` instead tries the lock
immediately and after waits of 100, 200, 400, 800, 1600, 3200, 6400 and then three of
10000 ms (11 attempts, 42,700 ms), returning the lock-held error when the last attempt
also finds it taken; other lock errors return at once. `InMemoryAuthStorageBackend` runs each callback on an owned copy of
the text with nothing held, so callbacks may reenter it and the last write wins; this
holds for the asynchronous operation too.
`AuthStorage::from_storage` accepts either, or any `AuthStorageBackend`.

## Configured values

Non-command values use a nonempty exact-name environment value or the unchanged
literal. Values beginning with `!` execute the remainder as a command. Successful
command results and absence are cached process-wide by the complete configured
string; concurrent misses share one initialization, including an absent result.
`clear_config_value_cache` removes the cached entries. Uncached and throwing
operations bypass that cache without replacing it.

`resolve_headers` omits empty or unresolved values. `resolve_headers_or_throw`
retains successful empty strings and stops at the first resolution error. Both
preserve literal header names and input insertion order, including `__proto__`.

Callers supply `ConfigValueOperations` for environment and command effects.
Native callers can use `ProcessConfigValueOperations` with a lazy configured-shell
selector. Unix uses the default shell without reading that selector. Windows
tries the configured shell first and falls back after selection, infrastructure
or missing-executable failure, not after a completed unsuccessful attempt.
Native execution ignores stdin/stderr, captures stdout, inherits environment and
working directory, and applies a 10-second process/output timeout. Timeout cleanup
kills and awaits the owned direct child; it does not promise descendant cleanup.
Command stdout is decoded lossily as UTF-8 and trimmed at its outer boundaries;
literal and environment values are not trimmed.

Guidance formatters take a documentation root without reading files. Authored
path joining belongs to [maestro-path](../crates/maestro-path/README.md).
See the [crate example](../crates/maestro-credentials/README.md) for a controlled
adapter. Browser callers supply their own operations; the native adapter is absent
from browser builds.
