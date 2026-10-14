# Native process libraries

`maestro-process` uses `process-wrap =10.0.1` on Unix only, with default features
disabled and `tokio1` and `process-session` enabled. Its license is
Apache-2.0 OR MIT. The enabled closure includes `nix =0.31.3` (MIT) and
`futures-executor =0.3.34` (MIT OR Apache-2.0).

The safe `ProcessSession::pre_spawn` hook configures the existing captured-child
spawn with a new Unix session. No wrapped child, group-wait loop or wrapper spawn
is used. Ordinary captured spawning and synchronous probes do not install the hook.

Tools use the existing `indexmap =2.14.2` (MIT OR Apache-2.0) to retain native
environment entry order and detached-PID insertion order.

Synchronous bounded capture uses the existing Tokio 1.53.2 `sync` feature for
one-shot termination requests through the shared waiter. The byte bound covers
combined retained stdout and stderr; it adds no separate process lifecycle.
