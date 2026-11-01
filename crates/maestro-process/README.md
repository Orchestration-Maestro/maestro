# maestro-process

Native child-process lifecycles, including [Linux pane terminals](../../docs/tools-process.md#pty-children).
Captured operations cover spawn, raw pipe collection, exit and
EOF observation, termination and reaping. A wait takes `WaitOptions`: the capture-only stderr policy and an optional
post-exit grace ending collection. Retained bytes are returned; streamed bytes
have already been delivered.
`Termination::TerminateThenKill` kills a child still running after its delay.

## Termination signals

`termination::register_signals` delivers each termination signal of the host to a
callback and returns the operation that stops delivery; calling it again changes
nothing. `termination::signals_for_host` names the signals delivered: termination
everywhere and a hangup off Windows. Delivery runs as tasks of the calling thread,
so a caller drives it from a runtime with the signal driver. A native mode keeps
its own exit policy. On Unix, removing the last live registration restores native
TERM/HUP default actions. Removing one of several registrations leaves the others
live. Reopening defers the retained prepared actions again; no stale preparation
is installed. Dropping a removal handle alone does not unregister it.

`DefaultSignalActions` prepares the native default action a repeated signal takes
once restored.

See the [process contract](../../docs/specs/maestro-port.md#crates-and-delivery-order).

## Command preparation

`Launch::with_context` collects the child environment and working directory
alongside the executable and arguments. Its operations resolve the executable
when they spawn it; they do not expose a mutable command builder.
On Windows, bare names search the child directory then `PATH`, unless
`NoDefaultCurrentDirectoryInExePath` is present (even empty). That variable skips
the default child-directory search, not explicit paths or an authored `.` in
`PATH`. Empty `PATH` members are ignored. Relative program paths and relative
`PATH` entries use the child directory. Absolute program paths
keep their location. Extensionless names try `PATHEXT` in order (default
`.EXE;.COM;.BAT;.CMD` when absent); names with explicit extensions are checked
without appending one. Supplied lookup variables replace inherited values,
including empty values; omitted keys inherit unless the environment is cleared.
Windows variable names are case-insensitive.

The standard library handles batch argument quoting. For a verbatim Windows
shell launch, only the final command-text argument bypasses quoting;
interpreter flags remain ordinary arguments. Callers choose inherited or
captured streams, deadlines and the existing background-console setting before
launching. The launcher reuses the capture and wait operations below.

`shell_quote` serializes POSIX operands. `quote_powershell_arg` may leave safe
PowerShell operands unquoted; `quote_powershell_literal` always quotes and is used
for expression-mode values such as clipboard paths.

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
`None` and zero disable it. Overflow stops collection, closes unfinished stdin
and force-kills/reaps the direct child through the same waiter, returning
`SpawnSyncError::Wait`; native
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

`exit_signal_name` names the signal that ended a child, returning `None` for a
numeric exit and for a signal this host does not name. `windows_shell_launch`
builds a Windows shell launch for callers that must run through one, without
parsing their command text.

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
