# Workspace daemon

The [daemon specification](specs/workspace-daemon.md) defines the application.
The transport leaf described here provides synchronous Unix sockets; application
wiring and environment selection are tracked in [path composition](https://github.com/Orchestration-Maestro/maestro/issues/1441).
The common command records live in [daemon protocol](daemon-protocol.md).

## Sockets

`maestro-ipc` owns native socket effects and consumes supplied paths. It reads no
environment variables and chooses no configuration root. Both transports use the
shared [JSONL codec](json.md#jsonl-framing); socket code defines no command set.

`socket_paths::client_socket_path` selects, in order:

1. The caller's explicit named-daemon client path, even when empty.
2. The client path derived from the supplied API socket override
   (`MAESTRO_SOCKET_PATH` at the application boundary).
3. The supplied client override (`MAESTRO_CLIENT_SOCKET_PATH`), even when empty.
4. `maestro-client.sock` inside the supplied daemon folder.

Derivation uses the API path's native file stem and parent: `api.sock` becomes
`api-client.sock`, `a.tar.gz` becomes `a.tar-client.sock`, and a path with no usable
UTF-8 stem uses `maestro-client.sock`. Relative paths stay relative; selection does
not canonicalize them. Socket filesystem operations resolve relative paths when
called.

Before binding, preparation creates parents, rejects a live listener with
`maestro server is already running (socket busy at {path})`, and removes stale
socket entries. A stale connection failure does not authorize deletion of a
regular file, directory or symlink: preparation preserves that entry and returns
`AlreadyExists`. A link to a live listener takes the live-listener branch.
Unclassified errors propagate unchanged.

Binding disables automatic pathname removal on listener drop. The caller records
`SocketFileIdentity` and can remove the pathname if its current device and inode
still match. A missing path succeeds. This metadata comparison followed by unlink
is not atomic; callers must coordinate pathname replacement with cleanup.

After binding, callers restrict the socket to owner read/write mode (`0600`)
through `socket_paths::restrict_socket_permissions`. Private listeners also need
this post-bind call on Unix. Write shutdown leaves the read direction usable.
Only an actual end-of-input read ends incoming traffic; pending command bytes are
not consumed by a disconnect probe.

Windows transport is tracked in [Windows sockets](https://github.com/Orchestration-Maestro/maestro/issues/1473),
and other Unix qualification in [Unix qualification](https://github.com/Orchestration-Maestro/maestro/issues/1472).
See [socket libraries](libraries/workspace-daemon.md) for dependency features and licences.

## Topology

`maestro-workspace` models workspaces, tabs, split layouts and pane viewport
state. A `WorkspaceTopology` owns the collection and one identity allocator.
Its clock is supplied by the caller; separate owners have independent counters.

Workspace handles and public tab and pane numbers stay stable when tabs move.
Closed numbers are not reused. Restore admission reserves saved numeric
identities and public counters before subsequent allocation, and assigns fresh
terminal identities.

Layouts keep a binary split tree, ratios and focus history. Closing the selected
pane returns to the remembered live pane, or a neighbouring leaf in tree order.
Resizing uses the nearest eligible divider, with an opposite-edge fallback.
Geometry uses cell rectangles without frontend decorations.

Creation and splitting take a consuming spawn callback. The callback receives
owned workspace, tab, pane and terminal identities; its result returns directly
to the caller without a thread-safety or cloning requirement. A failed split
restores the layout, focus history and viewport state. Failed tab creation keeps
its reserved tab number but does not consume a public pane number.

Pane transfers return their original state when insertion fails. The source
keeps its public pane mapping until the caller explicitly unregisters it. Taking
the sole pane can leave an empty source workspace for the caller to remove or
refill.

Repository status and native identity paths arrive as data. Labels use supplied
HOME and checkout observations without reopening paths. Metadata expiry is
explicit: callers supply an instant and request a sweep; reading values does
not expire them.

The topology API starts no terminal, background task or daemon listener. See the
[daemon design](specs/workspace-daemon.md) for the remaining runtime boundaries.

## Snapshot

[`maestro_workspace::persist`](../crates/maestro-workspace/src/persist/mod.rs)
loads and saves version-1 topology at supplied paths. Owned records retain
workspace/tab/pane names, public numbers, selection, working directories, split
axes and resume provenance. Screen history is stored separately and can be
removed by saving without history.

Saving writes pretty JSON without a trailing newline through the shared
[atomic replacement](locking.md#atomic-replacement) operation, following symbolic
links. Topology commits before history is saved or removed; failure on the second
file does not undo the first. Snapshot saves ignore postcommit directory-sync
advisories.

Missing files load silently as absent. Read, parse and unsupported-version failures
produce warnings and return no snapshot. Only version 1 is accepted; there is no
legacy reader or conversion. Topology classifies its raw header before decoding
workspaces, while history decodes its complete record before checking its version.
Unknown record members are ignored. Capture, restore and default-path selection
are separate operations, not part of these supplied-path functions.

## Pane processes

The Linux process layer provides owned terminals through `maestro_process::pty`.
A pane's caller supplies its executable, literal arguments, working directory,
environment policy and setup entry. The child becomes its terminal's session leader;
the master supports foreground-group queries and window-size changes.

The I/O actor owns the master. Input waits for capacity, while terminal replies and
geometry updates remain independent of that queue. During handoff, accepted input
drains before reads pause. Only a quiesced actor can duplicate its master. Rollback
resumes it; successful committed release closes the old owner without closing an
owned duplicate. An optional asynchronous exit hook finishes before the old master
is released. Graceful shutdown must keep draining critical notifications until
pending hooks finish, before destroying the runtime. The
[daemon loop delivery](https://github.com/Orchestration-Maestro/maestro/issues/1445)
owns that integration; destroying the runtime cancels outstanding hooks and releases
their terminal resources.

Shell builders retain login-shell custom commands, plain pane commands and
shell-evaluated scrollback editor selection. Foreground observations contain process
facts rather than application classifications. Application wiring is described by the
[workspace daemon contract](specs/workspace-daemon.md).
