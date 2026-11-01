# Native process libraries

`maestro-process` uses `process-wrap =10.0.1` on Unix only, with default features
disabled and `tokio1` and `process-session` enabled. Linux PTY setup also enables
`reset-sigmask` to clear inherited blocked signals before target execution. Its license is
Apache-2.0 OR MIT. The enabled closure includes `nix =0.31.3` (MIT) and
`futures-executor =0.3.34` (MIT OR Apache-2.0).

The safe `ProcessSession::pre_spawn` hook configures the existing captured-child
spawn with a new Unix session. No wrapped child, group-wait loop or wrapper spawn
is used. Ordinary captured spawning and synchronous probes do not install the hook.

Tools use the existing `indexmap =2.14.2` (MIT OR Apache-2.0) to retain native
environment entry order and detached-PID insertion order.

`Launch::with_context` overlays the pairs the caller supplies in order, so a
startup default placed first is replaced by any later caller pair.
`clear_environment` replaces the inherited environment, and a caller editing a
native command keeps its own removals. This owner never adds a variable of its
own, so an explicit caller choice always reaches the child.

Synchronous bounded capture uses the existing Tokio 1.53.3 `sync` feature for
one-shot termination requests through the shared waiter. The byte bound covers
combined retained stdout and stderr; it adds no separate process lifecycle.

Native `maestro-chat` external editing directly uses Tokio `=1.53.3`, with default
features disabled and `process` enabled, for the caller-built command accepted by
the application's inherited-terminal process bridge. This reuses the existing
locked Tokio process dependency closure; it adds no runtime or deadline policy.
Tokio is MIT licensed.

Native `maestro-chat` names pasted clipboard images with `uuid =1.27.0`, defaults
off with `std` and `v4`, the version and features the application already locks.
uuid is licensed MIT OR Apache-2.0.

Linux pane terminals use the existing `rustix =1.1.5` (Apache-2.0 WITH
LLVM-exception OR Apache-2.0 OR MIT), with defaults off and `std`, `process`,
`pty` and `termios` enabled; `pty` enables `fs`. Owned descriptors provide
close-on-exec duplication, terminal creation, controlling-terminal acquisition and
window-size operations without unsafe application code.

The process crate enables Tokio `net` for `AsyncFd` readiness and `signal` for
setup-time terminal signal registration. Its existing process, I/O, runtime,
channel and timer features remain enabled. `tracing =0.1.44` (MIT), with default
features, records pane identity and native actor errors. These features reuse the
locked dependency closure; no package version changes.

On Unix, `maestro-process` uses `signal-hook =0.3.18` (MIT OR Apache-2.0), with default
features disabled, only to give a signal its native default action: a termination signal
once graceful shutdown has started, and an interrupt once a suspended interactive mode
continues. Tokio keeps signal delivery. The package is already in the locked dependency
closure; no package version changes.

For job control on Unix, the process crate also enables Tokio `signal` to observe the
continue signal, and sends the stop signal to its process group through rustix's
`process` feature. Both reuse the locked dependency closure.

The process tests run their controlled children as Rust. On Unix the test-process owner
(`maestro-test-process`) depends on the existing `rustix =1.1.5` with `std` and `stdio`,
so a child replaces one of its standard descriptors with the null device without unsafe
code; the process tests share that one operation. The process tests also use
`nix =0.31.3` (MIT), defaults off with `signal`, the version and feature the process
wrapper already compiles, as a development dependency: a child blocks signals with its
safe `sigprocmask` before it replaces itself with the test executable. The `maestro`
composition tests inherit the same workspace pin and `signal` feature. Their checked
children block termination before replacing themselves with the test executable, then
wait with `sigwait` after reporting readiness. No package version changes.
