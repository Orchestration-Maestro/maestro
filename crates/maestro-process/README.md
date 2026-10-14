# maestro-process

Native captured child-process lifecycles: spawn, raw pipe collection, exit and
EOF observation, termination and reaping. A wait takes `WaitOptions`: the capture-only stderr policy and an optional
post-exit grace ending collection. Retained bytes are returned; streamed bytes
have already been delivered.
`Termination::TerminateThenKill` kills a child still running after its delay.

See the [process contract](../../docs/specs/maestro-port.md#crates-and-delivery-order).

## Synchronous capture

`spawn_sync(&mut command, SpawnSyncOptions::Capture { input, timeout, stdout, stderr, max_buffer_bytes })`
runs a caller-built Tokio command from synchronous code, including inside a
running runtime. Supplied bytes are written unchanged then stdin closes; absent
input selects the null device. `OutputMode` independently selects capture or the
null device for stdout and stderr. Discarded descriptors add no pipe-EOF wait.
Nonzero exits remain completed outputs.

An optional timeout covers input, exit and captured EOFs; `None` and zero disable
it. Expiry closes unfinished pipe futures and force-kills/reaps the direct child
unless cleanup fails. Descendants are not signalled. Native failures retain their
`io::Error` sources through `SpawnSyncError`; input failures wait for completion
unless a deadline wins.

An optional `max_buffer_bytes` bounds combined captured stdout and stderr.
`None` and zero disable it. Overflow stops collection and force-kills/reaps the
direct child through the same waiter, returning `SpawnSyncError::Wait`; native
cleanup failures can prevent reaping. Discarded streams do not consume the bound.

`SpawnSyncOptions::Inherit` instead inherits all three descriptors, waits without
a deadline, and returns the native status with empty captured streams. No shell is
selected; callers prepare their platform commands.

## Background launch

`spawn(command, input)` acknowledges native startup before returning a standard
thread handle. Its worker writes optional owned bytes, closes stdin, ignores
input errors and waits for the direct child. Stdout/stderr use the null device.
Joining observes completion after normal reaping; dropping the handle does not
cancel or join it. Startup errors return before a handle is delivered.

The application re-exports these original launch interfaces plus the tools
owner's [asynchronous execution](../../docs/tools-process.md#running-a-command),
on native targets only.

## Streaming and sessions

`wait_for_child_process` takes an optional fourth argument, a synchronous raw-byte
sink `FnMut(OutputStream, &[u8])`. The descriptor tag identifies stdout or stderr;
bytes stay unchanged. With `Some(&mut sink)`, newly consumed stdout and stderr chunks go to the
sink rather than retained output; bytes retained by an earlier wait are not
replayed. Streaming delivers new stderr even with `OutputMode::Discard`; that
mode still drops earlier retained stderr. `None` preserves capture and
stderr-discard behavior. The same waiter
owns exit, EOF, selected grace and native error handling in either mode.
The local shell ignores the tag and forwards chunks in observed interleaving order.

`CapturedChild::id()` exposes the PID before reaping. `spawn_detached(command)` consumes the builder and applies a
new Unix session to that spawn only; ordinary `spawn` and
`spawn_sync` remain unchanged. Unix forced process/group signals and Windows
`request_tree_kill` expose native effects; callers choose tree-cleanup policy.
`EXIT_STDIO_GRACE_MS` supplies the 100 ms local-shell grace.
