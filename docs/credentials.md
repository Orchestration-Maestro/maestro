# Credentials

`maestro-credentials` keeps stored credentials, resolves credential values and
ordered headers, and formats login guidance. Key selection and OAuth refresh are
separate, not delivered here.

## Stored credentials

`AuthStorage` accepts the stored records once, when it is created, and again on
`reload`; later reads and changes use that accepted snapshot. Missing or empty
stored text is an empty store. Text that is not a JSON object, or cannot be read,
locked or parsed, is a load failure: the accepted records are kept, the failure
is recorded for `drain_errors` (oldest first), and `set`/`remove` change only
accepted memory until a `reload` succeeds. Construction never fails.

`set` and `remove` change accepted memory first, then reread the stored text under
the backend's exclusion and change only that provider in it; records other writers
added stay in the file and are not imported into memory. A persistence failure is
recorded and nothing is rolled back. `logout` removes the stored record only.

Stored text is two-space JSON without a trailing newline. Providers keep their
stored order: a replaced record keeps its position, a new one is appended, and
`list`, `get_all` and the rewritten file follow it. `get` and `get_all` return
typed copies of complete `api_key` and `oauth` objects only; `list` and `has` cover
every stored record, whatever its shape, and rewrites keep records that do not
decode. Whole-number expiry times are written without a fraction.

`has_auth` is true for a runtime override (even empty), a stored record, a usable
provider environment value or a nonempty fallback key. `get_auth_status` reports
the first of stored, runtime (`--api-key`), environment (first populated variable
name) and fallback (`custom provider config`), without key values, helper runs or
refresh. See the [request authentication documentation](request-authentication.md) for
environment discovery.

## File and memory backends

`FileAuthStorageBackend` (native builds) keeps credentials at a caller-supplied
path, relative paths resolving against the working directory. Missing parent
directories are created with mode 0700 and a missing file is created, only while
the sidecar `<path>.lock` is held, containing `{}` with mode 0600 on Unix; writes
set mode 0600. The lock is tried ten times, 20 ms apart; other lock errors return
at once, and the lock is released after every outcome. File bytes are read as
lossy UTF-8. `InMemoryAuthStorageBackend` runs each callback on an owned copy of
the text with nothing held, so callbacks may reenter it and the last write wins.
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
