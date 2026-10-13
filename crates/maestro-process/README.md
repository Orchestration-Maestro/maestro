# maestro-process

Native captured child-process lifecycles: spawn, raw pipe collection, exit and
EOF observation, termination and reaping. A wait takes `WaitOptions`: whether stderr is kept and an
optional post-exit grace after which the bytes read so far are returned, and
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
The result is `SpawnSyncError::Timeout`. Other failures preserve their native
error as the source of `Infrastructure`, `Spawn` or `Wait`; see
[`wait_for_child_process`](src/child_process.rs) for wait errors.
