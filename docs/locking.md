# File locking

## Quick start

Use `maestro_lock::replace` to publish bytes in place with recovery, or
`atomic_replace` when unlocked readers must see a complete old or new file.
For a read-update-write transaction, acquire a `WriteLock`, read its `path()`,
then publish through its consuming `replace` method. Use the holder's consuming
`append` method to add bytes without replacement. Do not acquire a second lock for
the same target while holding the first.

## Acquisition

`acquire_write(path, through_symlinks)` waits until the target's lock is free.
`acquire_write_bounded` makes up to ten attempts, waiting 20 ms only between
contended attempts. `try_acquire_write` makes one attempt. The bounded and
single-attempt forms report contention as `TryLockError::WouldBlock`; other
failures retain their native errors.

The lock is `<target>.lock` beside the canonical target. Missing paths use their
deepest existing canonical ancestor plus the remaining components. The caller
selects whether to follow final symbolic links. Linked aliases share exclusion
when followed; different hard-link names are separate locks, so concurrent writes
through those names are outside this contract. Acquisition reads neither the home
directory nor the configuration root.

Acquisition creates the lock file; dropping the holder removes it. The operating
system releases a dead holder's lock, and the next holder reuses any file left
behind. Unix holders compare the locked file's device and inode with the lock
path before proceeding. Windows holders use delete sharing and delete-on-close,
then reopen the path after locking to reject a file already being deleted.
A symbolic link at the lock path is refused with `InvalidInput` before opening
it. A sharing violation or an access denial while the file exists counts as
contention. A directory at the lock path fails immediately. An access denial with
no entry is retried once immediately; ten consecutive access denials stop even
the waiting form.

Lock and recovery names append `.lock` and `.recovery` to the complete file name.
A name fits within 255 bytes on Unix or 255 UTF-16 units on Windows. Only names
that would exceed that limit are shortened: a cut file name at a character
boundary, a hyphen, the first 16 hexadecimal digits of its SHA-256, then the
suffix. The same naming operation bounds hidden record staging names containing
the process ID and nanoseconds.

## Recoverable byte publication

Before admitting a holder, acquisition replays any pending `<target>.recovery`
record beside the target. The record is bound by its name, so moving or copying
a directory with a pending record does not prevent replay. Its five-byte header
holds a creation-mode flag and a little-endian mode; an incomplete header or an
unknown flag is refused. A clean read-only guard does not sync the directory.
Each holder performs at most one caller-requested write. `WriteLock::append`
syncs recovery absence before opening the target, so an older record cannot
resurrect after later appends. Replacement's staged-record directory sync gives
the same protection before its first target write. Append creates a missing
target with native default permissions; it does not sync appended bytes.
Writable session loaders recover and read under exclusion before selecting
history or parent IDs. Where the session directory is missing or refuses the lock
file, session reads and appends run without it; rewrites still require it.

Replacement writes the intended bytes to an exclusively created private file
beside the target, syncs it, renames it to the recovery name and syncs the
directory. On Windows, unsupported directory sync is tolerated, relying on the
filesystem journal for the rename. Only then does it truncate and write the
target in place. It syncs the target before removing the record and syncing the
directory again. An interrupted writer leaves the record for the next holder to
complete, not roll back.

In-place writing preserves ownership, ACLs, native permissions and hard links.
Existing read-only targets still refuse writable opens. Creation mode applies
only to missing targets. Default creation permissions are unchanged. With an
explicit creation mode, a new target starts owner-only and writable. Its bytes
are synced before the requested mode is applied and synced. Replay skips the
target write when a staged read-only creation's bytes and mode already match;
it syncs the verified mode before syncing the parent and removing the record.
On Windows, this verified creation temporarily becomes writable for flushing,
then regains read-only before another flush. A failure restores read-only and
retains the record. Other targets still require a writable open.
Uncoordinated readers can observe an in-progress write; recovery is guaranteed
on the next coordinated acquisition, not on an arbitrary native read.

Settings and credentials hold one write lock across read, update and replace.
They keep their bounded synchronous and scheduled asynchronous acquisition.
Serialization and scheduling stay with callers; there is no background recovery
service. Standalone account files, keybinding writes and session rewrites use
the same publication owner.

## Atomic byte publication

Workspace snapshots and detection catalogs use `maestro_lock::atomic_replace` so
unlocked readers see complete old or new files. It creates missing parents,
writes and syncs a temporary file beside the target, closes it, renames it over
the target and syncs the parent directory. Precommit failures remove the temporary
file; a parent-sync error after rename is returned as a committed warning.

The caller chooses whether to follow final symbolic links and whether to use a
fixed temporary extension or a hidden name containing the target name, process
ID and nanoseconds. Writers hold canonical target exclusion through publication.
A stale fixed temporary is unlinked without following it, then the new temporary
is created exclusively. An intervening creator is left untouched and publication
fails. On Unix, creation uses existing target permission bits from the start;
final permission bits are retained before writing bytes.
Renaming replaces the inode: hard links keep their old bytes, while ownership and
ACLs are not preserved. Lock acquisition completes any pending recovery record
before atomic publication begins; the new atomic write does not stage one.

## Examples

Settings and credential transactions use bounded acquisition to read fresh text
before an update. A session rewrite waits for exclusion, compares the current
bytes with its expected hash, then publishes through the held lock. Session
appends consume the guard through `append`, which owns prewrite synchronization.
A snapshot save uses atomic publication because its readers do not acquire a lock.
