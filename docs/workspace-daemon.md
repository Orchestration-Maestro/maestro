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
