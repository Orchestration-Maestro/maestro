# Component operating environment

The native host supplies one owned WASI context and resource table to both the
standard-library 0.2 interfaces and the native asynchronous 0.3 interfaces.
The existing Store thread runs their I/O and timer drivers; application
capabilities keep their original local owners.

Environment keys and values are copied from the native environment, including
empty values. Native strings use replacement for invalid Unicode. The host
removes any inherited `MAESTRO_EXTENSION_PATH` entry before adding the selected
artifact path for this instance. Neither another instance nor the inherited
value can replace it. Other entries are unchanged.

Stdin, stdout and stderr are inherited separately. Guest buffers still need to
be flushed when immediate visibility matters. Network name lookup, TCP and UDP
use the user's network without an added address filter.

Every component lists one filesystem root, `/`, for reading and writing,
rather than the extension directory. For a path below it, the host opens the
native directory that holds the path with the user's own access and finishes
the operation there. The operating system therefore resolves every directory
on the way, as it does for a native process: a drive mounted after the
extension loads, any network share, junctions, and links with absolute
targets. On Unix a user may be allowed to search a directory but not list it.
The host cannot open such a directory, so a path below it is resolved one name
at a time from the root, where an absolute link is refused.

The host supplies the native current directory when it can observe it. Guest
export startup applies that observation once to Rust’s standard-library cwd
before the author factory runs; its libc would otherwise start at `/`. Failure
to observe cwd does not prevent loading an accessible absolute artifact: no
initial directory is supplied, and the guest keeps its libc default.
Filesystem operations still report ordinary operating-system failures.

The component's standard library treats only `/` as a separator and only a
leading `/` as absolute. A native Windows path such as `C:\Users\me\notes.txt`
is therefore joined to the cwd, and Windows reads its `C:` segment as the
current directory of that drive, so the call can fail or open a different file.
Host paths, including `ctx.cwd()`, the agent directory and the results of
`tools::path_utils::resolve`, keep the native spelling. Convert one with
`tools::path_utils::guest_path` just before a `std::fs` call and keep the
native text for display and records:

```rust,ignore
let path = resolve(&api, &[&ctx.cwd()?, &params.path])?;
let bytes = std::fs::read(guest_path(&api, &path)?)?;
```

The conversion uses `/` separators and adds a leading `/` to a drive path, so
`C:\Users\me` becomes `/C:/Users/me`; a verbatim `\\?\` prefix on a drive
path is dropped. A share `\\server\share\dir` becomes
`/?/UNC/server/share/dir`, and any other verbatim or device path keeps its
prefix as `/?/`. No Windows file name is `?`, so these forms never collide with
another absolute path: `/tmp/x` or `\tmp\x` names that path below the current
drive's root, as it does natively. Names keep their spelling, and Windows
itself decides which spellings name the same drive or share, so shares named
`ß` and `SS` stay apart. Relative paths keep their form. Resolve a
drive-relative path such as `C:notes.txt` first; its meaning depends on
per-drive state the component does not have. Unix paths are returned unchanged.
The cwd is supplied in the same spelling, for example `/C:/Users/me/project`,
and the 0.3 interfaces list the same single root, so a component that opens
through them passes the converted path without its leading `/`.

Links below the root behave as they do natively. Relative and absolute
symbolic links and junctions are followed, including links to directories.
Reading a link returns its target in the component spelling, and creating one
takes a target in that spelling, absolute or relative; on Windows a link to a
directory is created as a directory link. Rust's standard library has no
stable call that creates a link on the component target, so a component
creates one through the 0.3 `symlink-at` operation. Rust's `std::fs` always
passes paths through the root. A directory that a 0.3 caller opens itself
keeps the standard rules for paths below it, so an absolute link there is
refused; pass the whole path through the root instead. WASI paths are strings: a
followed link whose final native name is not UTF-8 returns `illegal-byte-sequence`
rather than opening a replacement-character spelling. Atomic saves and modes use the native-I/O capability described below. Final application-mode output routing remains separate from these
host capabilities.

## Link checks

The real-component probe uses ordinary Rust `std::fs` through WASI 0.2 and
`wasi:filesystem` 0.3 `open-at`/`read-via-stream` through actual extension
loading and callback dispatch. On an unprivileged Linux host:

| Observation | Rust / WASI 0.2 | Standard 0.3 | Native control |
| --- | --- | --- | --- |
| Direct path, relative file/directory links outside cwd | Exact bytes | Exact bytes | Not separately exercised |
| Absolute file/directory links outside cwd | Exact bytes | Exact bytes | Exact bytes |
| Later target update through the same links | Updated bytes | Updated bytes | Updated bytes |
| Dangling absolute link | Not found | `NoEntry` | Not found |
| Genuinely unreadable direct target | Permission denied | `Access` | Permission denied |
| File below a directory the user may search but not list | Exact bytes | Exact bytes | Not separately exercised |
| Target of an absolute link | Component spelling | Component spelling | Native target |
| New link with an absolute or relative target | Not exercised | Native target, exact bytes | Native target |

The fixture keeps the original 0.3 error code beside its semantic error label,
so an operating-system denial stays distinguishable from other failures.

On Windows, the same component, loaded before each of them existed, writes and
reads a file through a drive mounted with `subst`, an administrative share in
the plain and `\\?\UNC\` spellings, a junction on each of those drives, a
path that starts at the drive root, an absolute file link and a path longer
than 260 characters through a directory link, in both generations. It also
reads and creates link targets as in the last two rows above. macOS behavior is
not separately qualified.

## Detached children

A child launched with `SpawnOptions { detached: true, .. }` can outlive its
initiating callback and an extension reload. Reload releases the old guest's
controls; it does not terminate the native child. The application's existing
process collector continues independently and reaps the child when it exits.
A retained native waiter observes that collector's completion without keeping
the old guest alive. This requires the application's local executor to remain
running; it is not a guarantee that a child survives application shutdown.

The Linux real-host check reloads an `AgentSession`, verifies fresh guest
controls, exchanges a new nonce with the original PID over an independent TCP
connection, then releases the child and observes its original native waiter.
This qualification is separate from the broader component example checks.

## Supplied stdout

`artifact_factory_with_stdout(path, widgets, installation, writer, terminal)`
retains a standard `Write + Send` sink across initialization and later
callbacks. Both standard output generations share that sink and report the
supplied terminal observation. Closing one guest output view does not close the
sink. The ordinary `artifact_factory` continues to inherit stdout.

`ProcessTerminal::output_writer()` returns a cloneable, `Send + Sync` writer
while queued terminal output runs. Each write copies its bytes once into the
same ordered ingress as complete host frames. Flush checks admission; it does
not wait for physical delivery. Stop closes admission, drains accepted bytes
through the existing partial-write owner, then restores descriptor flags.
Retained handles return `BrokenPipe` after closure. Stopped terminals and
synchronous regular-file destinations return no handle.

The executable's final selection of this supplied route is not wired here.
The ordered-output guarantee applies to active queued destinations, not to
inherited stdout redirected to a regular file.

## Native file ownership

The native-I/O file operations belong to the application's
`extension_native_io::files` module. Byte saves use `maestro-lock` for atomic
publication, preserving existing permissions and applying a requested mode to
new files. The guest forwards operands without implementing a save algorithm.
The application selects final-link policy and reports native errors; a committed
save's directory-sync warning is separate from a precommit failure. Permission
bits are native Unix observations, not a Windows POSIX or ACL guarantee.

### Author example

```rust,ignore
let io = api.native_io();
io.create_dir(&private_directory, 0o700)?;
if let Some(warning) = io.save_file(&private_file, b"\0\xff", 0o600)? {
    // The bytes are committed; report the durability warning without retrying.
    eprintln!("{warning}");
}
let mode = io.file_mode(&private_file)?;
io.set_file_mode(&private_file, mode & 0o777)?;
```

Pass authored native paths directly to these methods, including Windows paths;
`guest_path` is only for guest standard-library calls. Save accepts empty or
binary byte slices. Existing native permissions win over the creation mode.
New Unix temporary siblings are never more permissive than the requested mode;
publication does not expose a permissive intermediate destination. Saves follow
final links, including relative links and dangling targets, and retain the link
entry. The lock owner's forty-link bound and native parent-creation behavior
apply. A directory is created recursively with native creation/umask semantics;
existing directories are not chmodded. Mode reads include the Unix file-type
bits, as native metadata does. Mode changes follow links.

Errors retain their native diagnostic text across the component boundary.
`Err` from save precedes publication; `Ok(Some(warning))` means publication
succeeded but directory durability could not be confirmed. Do not retry that
write as though it failed. Directory creation and mode operations return their
native errors. Non-Unix mode reads/changes return unsupported. On Windows save
uses the native lock owner's permission preservation and owner-write/read-only
mapping for a missing file; requested directory modes do not configure ACLs.
No POSIX privacy guarantee is made there.

### Native file checks

The authored `native-files` component command is loaded through ordinary native
startup and dispatch, then checked with native filesystem observations on Linux:

| Operation | Observation |
| --- | --- |
| New binary 0600 file, empty rewrite of a 0640 file | Exact bytes and requested/preserved modes |
| 0700 directory, mode read and change to 0750 | Native metadata matches |
| 255-byte ASCII leaf | Exact saved bytes |
| Relative final link to existing or dangling target | Link retained; target bytes and mode match |
| Non-directory parent or genuinely unwritable parent | Native error; original bytes/mode unchanged; no stray sibling |

The real-denial case uses a directory the test process cannot write; a privileged
host reports that this premise cannot be exercised. The lock owner's secure
creation tests check the temporary file's permissions before publication under
permissive and restrictive Unix umasks. The extension path delegates directly to
that owner rather than duplicating its algorithm. Windows and macOS execution
of these new extension operations has not been separately qualified.
