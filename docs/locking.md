# File-lock acquisition

`maestro_lock::acquire` returns the supplied file after acquiring an exclusive lock.
It makes at most ten attempts, waiting 20 ms only between contended attempts.
The final contention error or the first non-contention error is returned unchanged.
The waits are requested delays, not a wall-clock completion deadline.

Callers own opening, permissions, persistence and unlocking. Settings and credentials
delegate synchronous acquisition here; asynchronous credential acquisition remains
with [credentials](credentials.md). Browser compilation does not provide browser
file locking.

## Atomic replacement

`maestro_lock::replace` publishes supplied bytes through an exclusively created
sibling file with a short name independent of the target leaf. It writes and
syncs that file, closes it, renames it over the target,
then opens and syncs the parent directory. Precommit failures return the native
error and remove only a temporary file created by this call.

Replacement preserves the effective existing file's native permissions. On Unix,
the temporary file is created with those permission bits filtered by the umask;
the exact bits are restored after writing and before sync and publication.
`Replace::creation_mode` selects permission bits for a missing file. Existing
native permissions take precedence. On Unix the requested bits are passed to
exclusive creation, so the sibling is never more permissive than requested;
any bits removed by the umask are restored before publication. Without a
selection, new files use the platform's normal creation permissions.

On Windows, creation mode uses only the owner-write bit: an absent bit makes
the published file read-only. Other bits are ignored; no ACL is configured.
Replacing a symbolic-link entry does not copy permissions from its referent.
Permissions here are those represented by `std::fs::Permissions`, not ownership
or additional access-control metadata.

`Replace::through_symlinks` chooses whether to follow final-component symbolic
links. Relative links resolve against their parent; dangling targets are allowed.
A chain still unresolved after 40 links fails before temporary-file creation.
When following is disabled, publication replaces the supplied directory entry.

After rename, publication has committed. Unexpected directory-open or sync errors
are returned as `Committed::dir_sync`, not as failed writes. Unsupported,
invalid-input and permission-denied directory sync errors are tolerated.
Serialization, logging and save scheduling remain with callers. The operation
provides neither a multi-file transaction nor locking between concurrent writers.
