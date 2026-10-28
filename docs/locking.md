# File-lock acquisition

`maestro_lock::acquire` retains bounded acquisition of a caller-opened file:
up to ten attempts, with 20 ms between contended attempts. Waiting acquisition
uses the operating system's exclusive file lock. Exiting releases ownership.

## Recoverable byte publication

The lock owner resolves the write target and hashes its canonical native path
with SHA-256. For a missing file it uses the canonical parent plus the leaf.
Lock and recovery files live in `~/.maestro/state/file-locks`, never under a
configuration-root override. Home selection uses `HOME` on Unix when present,
otherwise the account home directory. Processes sharing a target must share
that home; overriding it selects a different lock namespace.

`maestro_lock::replace` and path-based acquisition share that identity and
recovery owner. Before any append or rewrite, the holder replays a pending
recovery copy, refusing a mismatched target identity. Each holder performs at
most one caller-requested write.

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
before recovery deletion. Symbolic-link policy remains with the caller.
Uncoordinated readers can observe an in-progress write; recovery is guaranteed
on the next coordinated acquisition, not on an arbitrary native read.

Settings and credentials retain their bounded acquisition and existing in-place
writes. Workspace snapshots, detection catalogs and session rewrites use the
shared recoverable publication operation. Serialization and scheduling remain
with callers; no background recovery service is added.
