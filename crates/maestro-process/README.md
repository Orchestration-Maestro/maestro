# maestro-process

Native captured child-process lifecycles: spawn, raw pipe collection, exit and
EOF observation, termination and reaping. A wait takes `WaitOptions`: the capture-only stderr policy and an optional
post-exit grace ending collection. Retained bytes are returned; streamed bytes
have already been delivered.
`Termination::TerminateThenKill` kills a child still running after its delay.

See the [process contract](../../docs/specs/maestro-port.md#crates-and-delivery-order).

## Synchronous capture

`spawn_sync(&mut command, timeout)` runs a caller-built Tokio command from
synchronous code, including inside a running runtime. It keeps the caller's
program, arguments, directory, environment and platform flags. Stdin and stderr
use the null device; the result contains raw stdout, the actual exit status and
empty stderr. An unsuccessful status is still a completed output.

The caller's timeout covers both child exit and stdout EOF, without a post-exit
grace. On expiry, unfinished readers close; the direct child is force-killed only
if still running and reaped unless killing fails. Descendants are not signalled.
The result is `SpawnSyncError::Timeout`. The non-timeout variants
`Infrastructure`, `Spawn` and `Wait` expose an `io::Error` source; see
[`wait_for_child_process`](src/child_process.rs) for wait errors.

## Streaming and sessions

`wait_for_child_process` takes an optional fourth argument, a synchronous raw-byte
sink. With `Some(&mut sink)`, newly consumed stdout and stderr chunks go to the
sink rather than retained output; bytes retained by an earlier wait are not
replayed. Streaming delivers new stderr even with `StderrMode::Discard`; that
mode still drops earlier retained stderr. `None` preserves capture and
stderr-discard behavior. The same waiter
owns exit, EOF, selected grace and native error handling in either mode.

`CapturedChild::id()` exposes the PID before reaping. `spawn_detached(command)` consumes the builder and applies a
new Unix session to that spawn only; ordinary `spawn` and
`spawn_sync` remain unchanged. Unix forced process/group signals and Windows
`request_tree_kill` expose native effects; callers choose tree-cleanup policy.
`EXIT_STDIO_GRACE_MS` supplies the 100 ms local-shell grace.
