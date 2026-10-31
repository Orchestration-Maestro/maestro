# File-lock acquisition

`maestro_lock::acquire` retains bounded acquisition of a caller-opened file:
up to ten attempts, with 20 ms between contended attempts. Waiting acquisition
uses the operating system's exclusive file lock. Exiting releases ownership.

## Recoverable byte publication

The lock owner resolves the write target and hashes its canonical native path
with SHA-256. For a missing path it uses the deepest existing canonical ancestor plus the
remaining components.
Lock and recovery files live in `~/.maestro/state/file-locks`, never under a
configuration-root override. Home selection uses `HOME` on Unix when present,
otherwise the account home directory. Processes sharing a target must share
that home; overriding it selects a different lock namespace. On the first
acquisition in each process, Maestro syncs its three state directories and the
home directory before admitting a writer. It never opens ancestors above home.
A failed sync is retried on the next acquisition. Different hard-link
names are separate identities, so concurrent writes through them are outside this
contract.

`maestro_lock::replace` and path-based acquisition share that identity and
recovery owner. Before any append or rewrite, the holder replays a pending
recovery copy, refusing a mismatched target identity. Each holder performs at
most one caller-requested write. Writable session loaders, including fork sources,
recover and read under the same exclusion before selecting history or parent IDs.

Replacement writes the intended new bytes to an exclusively created private
file in the lock root, syncs it, renames it to the recovery name and syncs the
root. On Windows, the recovery file is flushed but unsupported directory sync
is tolerated, relying on the filesystem journal for the rename. Only then does it truncate and write the target in place. It syncs the
target before deleting the copy and syncing the root again. A failed or killed
writer leaves the copy for the next holder to complete, not roll back.

In-place writing preserves ownership, ACLs, native permissions and hard links.
Existing read-only targets still refuse writable opens. Creation mode applies
only to missing targets. Default creation permissions are unchanged. An explicit
creation mode is recorded in the recovery header: a new target starts owner-only
and writable, its bytes are synced, then the requested mode is applied and synced
before recovery deletion. Replay skips the target write when a staged read-only
creation's bytes and mode already match: its bytes were synced before that mode was applied.
It syncs the verified target's final mode before syncing the parent and removing
the recovery copy. On Windows, flushing needs a writable handle: only for this
verified creation, replay temporarily clears read-only, flushes the bytes,
restores read-only and flushes again. A failure restores read-only and retains
the recovery copy. Other targets still require a writable open. Symbolic-link policy remains with the caller.
Uncoordinated readers can observe an in-progress write; recovery is guaranteed
on the next coordinated acquisition, not on an arbitrary native read.

Settings and credentials retain their bounded acquisition of the sidecar lock, and
publish their text through the shared recoverable publication operation, as do the
standalone account file and session rewrites.
Serialization and scheduling remain with callers; no background recovery service is added.

## Atomic byte publication

Workspace snapshots and detection catalogs use `maestro_lock::atomic_replace` so
unlocked readers see complete old or new files. It creates missing parents,
writes and syncs a temporary file beside the target, closes it, renames it over
the target and syncs the parent directory. Precommit failures remove the temporary
file; a parent-sync error after rename is returned as a committed warning.

The caller chooses whether to follow final symbolic links and whether to use a
fixed temporary extension or a hidden name containing the target name, process
ID and nanoseconds. Existing permission bits are applied before writing bytes.
Renaming replaces the inode: hard links keep their old bytes, while ownership and
ACLs are not preserved. This operation does not acquire the recoverable writer's
lock or replay its recovery records.
