## Approved footer-effects libraries

On Unix, `maestro-app` uses Tokio `=1.53.3` (defaults off; `rt`, `time`,
`process`) for driving shared Git captures and reference-table polling on a
caller-driven `LocalSet`, plus `num-traits =0.2.19` (defaults) for timestamp
conversion without integer-millisecond truncation. Tests add Tokio `rt-multi-thread`
`sync` and `test-util` for controlled clock tests. Tokio is MIT and num-traits is MIT OR Apache-2.0. Notifications and
one-shot timers reuse `maestro-watch`; no notification library is duplicated in
the application. Git process lifecycles reuse `maestro-process` through a native-only workspace
edge. Launch exports from `maestro-tools` are native-gated; its shared key helpers
remain available on browser targets. Other targets supply `FooterOperations` without these native
process/stat edges.

Unix tests also use the existing rustix `=1.1.5` (defaults off; `std`, `process`,
Apache-2.0 OR Apache-2.0 WITH LLVM-exception OR MIT). Its non-reaping status
observation synchronizes the footer EOF test with direct-child reaping before
advancing the controlled clock.
