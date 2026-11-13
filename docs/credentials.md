# Credentials

For login, API keys and cloud configuration, start with [provider setup](providers.md).

`maestro-credentials` keeps stored credentials, resolves credential values and
ordered headers, selects request keys, logs in to OAuth providers, refreshes
expired OAuth tokens and formats login guidance.

The typed credential and status records (`ApiKeyCredential`, `AuthCredential`,
`AuthStorageData`, `AuthSource`, `AuthStatus` and `OAuthCredentials`) are defined
in `maestro-interchange` and re-exported here unchanged, so extensions use the same
types. Their ordinary serialization uses the stored field names and the snake-case
source names; it is not the stored-file writer, which stays with `AuthStorage`.

## Stored credentials

`AuthStorage` loads stored records when it is created and on `reload`. Every stored
read (`get`, `get_all`, `list`, `has`, `has_auth`, `get_auth_status` and `get_api_key`)
rereads the document under exclusion. The one exception is a credential file that
still has the size, modification time, device, inode and change time of the last read:
the remembered text answers without taking the lock. On Unix, every write moves the
change time (at the filesystem's timestamp resolution), even when it keeps the size or
restores the modification time, so a long-lived storage still follows what another
process stores or removes. Where the platform reports no change time, two equal-size
edits inside one timestamp tick look the same. A successful reread replaces the accepted
records, even when the text is unchanged. An acquisition, read or parse failure keeps
the accepted records and records nothing. Missing or empty text is an empty store;
a non-object JSON value is unparsable.

A reread never clears a recorded load failure. Only a successful `reload` or locked
refresh reread clears it. A failed load keeps the accepted records, records the
failure for `drain_errors` (oldest first), and blocks persistence by `set` and
`remove`; they change accepted memory only. Construction never fails.

`set` and `remove` change accepted memory first, then reread the stored text under
the backend's exclusion and change only that provider in it; records other writers
added stay in the file, and a successful write accepts the document it wrote. A
persistence failure is recorded and nothing is rolled back; the change stays in accepted
memory until the next successful read. `logout` removes the stored record only.
If a login or logout write fails, the next successful read follows the unchanged
file: a refused addition is absent and a refused removal is still present. The
write failure remains available through `drain_errors`.

Text this owner writes is two-space JSON without a trailing newline; reloading
existing text never rewrites it. Accepted records keep their order: a replaced
record keeps its position, a new one is appended, and `list` and `get_all` follow
it. A `set` or `remove` rewrite keeps the order of the freshly reread file, which another
writer may have changed, and the order of every reread follows the file. `get` and `get_all` return
typed copies of complete `api_key` and `oauth` objects only; `list` and `has` cover
every stored record, whatever its shape, and rewrites keep records that do not
decode. Expiry times of typed credentials you supply are written without a fraction when
whole; preserved stored records keep their numeric values, written in the JSON serializer's spelling (for example `1e3` becomes `1000.0`), not their original spelling.

`has_auth` returns `Ok(true)` for a runtime override (even empty), a stored record, a usable
provider environment value or a nonempty fallback key. For Cloudflare, an API-key
source must be nonempty; stored OAuth credentials count when an OAuth handler is
registered, without key extraction or refresh. The catalog also checks
[required account values](providers.md#cloudflare) on the API-key path.
`get_auth_status` reports
the first of stored, runtime (`--api-key`), environment (first populated variable
name) and fallback (`custom provider config`), without key values, stored helper
resolution or refresh. Both operations return a `Result`: a consumed fallback
resolver failure propagates unchanged. `set_fallback_resolver` accepts an owned
callback returning `Result<Option<String>, OAuthError>`; it runs without a storage
lock. See the [request authentication documentation](request-authentication.md) for
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
runtime override and stored key are resolved before it is built. A fallback
failure returns to the caller, including after an empty refresh, without entering
`drain_errors`; disabling fallback avoids invoking that callback. Presence ignores
an empty fallback key, while request lookup preserves `Some("")`.

### Refresh and login

An expired token is refreshed through the backend's `with_lock_async` (the file adapter
holds the write lock beside the canonical file meanwhile; the memory adapter
serializes awaited callbacks, see below). Lock acquisition uses the same randomly
lengthened schedule as stored rereads. The stored text is reread and its records
are accepted, clearing a recorded load failure.
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
path, relative paths resolving against the working directory. Only this backend
creates the parent directories, with mode 0700, and a missing file is created, only while
shared write lock `<target>.lock` beside the canonical target is held, containing
`{}` with mode 0600 on Unix. Synchronous and asynchronous transactions follow
symbolic links before choosing the lock path and hold that one lock across read,
update and replace. A write narrows an existing file to mode 0600, then publishes
through the held lock's
[recoverable byte publication](locking.md#recoverable-byte-publication) at mode
0600. The text is written in place, so a hard
link to the file sees the new bytes. A file its owner made read-only is refused and left
unchanged. A write cut short is completed, not rolled back, by the next coordinated writer.
The crate exports no writer function. The startup conversion of legacy credentials
and the standalone account file publish through the application's own helper, which
narrows and writes through the same recoverable publication. Both the backend and
that helper narrow through
[the shared in-place writer's](locking.md#recoverable-byte-publication)
`narrow_existing`, so the mode has one implementation. The
conversion creates the directory chain of the `auth.json` path first; the account
file creates none, so a missing directory is a write error.
Construction, `reload`, `set` and `remove` use
[the shared bounded file-lock operation](locking.md): ten attempts, 20 ms apart.
The lock file is removed after every outcome, including when a pending asynchronous
operation is dropped. File bytes are read as lossy UTF-8.

`AuthStorageBackend::read_waiting` reads text under the adapter's exclusion without
writing. Its provided implementation captures the text through `with_lock`, so
existing adapters need no additional method. The file adapter overrides it to wait
on the same schedule as `with_lock_async` before initializing and reading the file.
The first attempt is immediate. After contended attempt n, for n from 0 to 9, the
wait is `min(round((1 + u) * 100 * 2^n), 10000)` ms, using a fresh draw u in `[0, 1)`.
Random lengthening comes before rounding and the cap. An eleventh contended attempt
returns the lock-held error; any other acquisition error returns immediately.
The ten waits total 42,700 to 52,600 ms. `with_lock_async` awaits them;
`read_waiting` blocks the calling thread, including a current-thread async runtime,
for up to 52.6 s.

`InMemoryAuthStorageBackend` runs a synchronous callback on an owned copy of the
text with nothing held, so it may reenter the backend and the last write wins. The
asynchronous operation `with_lock_async` runs callbacks one at a time: each reads
the text the previous one stored, so two providers changed at once both stay. An
awaited callback must not reenter `with_lock_async`.
`AuthStorage::from_storage` accepts either, or any `AuthStorageBackend`.
Every constructor installs the built-in login providers into the
[provider registry](models/oauth.md#provider-registry), so `login` and token refresh find
`anthropic`, `github-copilot` and `openai-codex` on a fresh store.

## Configured values

Non-command values use the nonempty value of the environment variable the
configured value names, matched by the platform's variable-name rules, or the
unchanged literal. Values beginning with `!` execute the remainder as a
command. Successful command results and absence are cached process-wide by the
complete configured string; concurrent misses share one initialization,
including an absent result.
`clear_config_value_cache` removes the cached entries. Uncached and throwing
operations bypass that cache without replacing it.

`resolve_headers` omits empty or unresolved values. `resolve_headers_or_throw`
retains successful empty strings and stops at the first resolution error. Both
preserve literal header names and input insertion order, including `__proto__`.

Each call reads the value of the variable a configured value names from the
process environment.
Callers supply `ConfigValueOperations` for command runs.
Native callers can use `ProcessConfigValueOperations` with a lazy configured-shell
selector. Unix uses the default shell without reading that selector. Windows
tries the configured shell first and falls back after selection, infrastructure
or missing-executable failure, not after a completed unsuccessful attempt.
Native execution uses the [shared synchronous capture](../crates/maestro-process/README.md#synchronous-capture)
with a 10-second timeout. A timeout completes without a value or fallback.
Command stdout is decoded lossily as UTF-8 and trimmed at its outer boundaries;
literal and environment values are not trimmed.

Guidance formatters take a documentation root without reading files. Authored
path joining belongs to [maestro-path](../crates/maestro-path/src/lib.rs).
See the [crate example](../crates/maestro-credentials/README.md) for a controlled
adapter. Browser callers supply their own operations; the native adapter is absent
from browser builds.

## Extension credential handles

A guest acquires `AuthStorage` through `ModelRegistry::auth_storage()` or wraps a
supplied `AuthStoragePort`. Clones share the acquired owner, independently of the
context or catalog handle. Stored reads and changes, runtime overrides, reload,
logout, authentication metadata and drainable errors use the native credential
algorithms above. Key resolution retains the original owner and command effects
until it settles. Host and record-decoding failures are recoverable
`ExtensionResult` errors; persistence failures still drain from the owner.

Guest `create` and `in_memory` constructors use the host selected by the supplied
`ExtensionAPI`. An omitted file path uses the application's lazy default;
`in_memory` performs no file I/O. `from_storage` retains the author's raw-text
backend and lends it the native update for the duration of the call. Synchronous
and awaited updates preserve no replacement versus an empty replacement and
propagate the original error. A pending update retains its native borrow and
selected backend until settlement.

`set_fallback_resolver` replaces the retained resolver without changing an already
selected invocation. Synchronous metadata reads return the callback's actual value
before returning to their caller. Native constructors and backend algorithms are
unchanged. Component-backed native callbacks run on their creating local pump;
they are not an off-thread credential backend. Stored presence checks (`has`) use
the same callback-capable route as the other stored reads, so an authored backend
can complete its reread before the check returns. The registration facade does not
retain the bound session catalog. Acquired catalog and credential handles retain
their owners; resolver captures release after their final owner and selected
invocations end.

A rejected constructor leaves its selected file untouched. Catalog construction
rejects credentials from another host before resolving the model path. Rejected
backend or resolver admission releases its captures without retiring a sibling
credential resource.
