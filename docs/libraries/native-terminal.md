## Approved native terminal libraries

On every non-browser target, `maestro-tui-crossterm` uses Tokio `=1.53.3` (defaults off; `rt`,
`time`) for its caller-driven local task set, `chrono =0.4.45` (defaults off; `clock`) for log
file names and the runtime host's UTC time, and `rand =0.10.3` (defaults) for the host's log
nonce. On Unix it adds rustix `=1.1.5` (defaults off; `std`, `event`, `fs`, `stdio`,
`termios`) on the actual standard descriptors and Tokio's `net`, `signal` and `macros`.
Rustix is Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT, Tokio is MIT and chrono and rand
are MIT OR Apache-2.0. The crate does not use crossterm on Unix, because crossterm prefers the
controlling terminal over the redirected descriptors. On Windows it adds `crossterm_winapi =0.9.1`
(MIT, no features) for the console modes, the console input records and the console window
size, used through its safe interface only, and Tokio's `macros`; it does not use crossterm
there either, since the console modes it needs are set directly. Its tests add Tokio `test-util` and
`sync` on every non-browser target, plus rustix `pipe`, `pty` and `process` on Unix. No library type appears in a public
interface except Tokio's `LocalSet`, which the caller supplies to `ProcessTerminal::new`.

The Unix [terminal examples](../terminal/examples.md) add dev-only base64 `=0.23.1`
(defaults off; `std`) for image bytes, serde `=1.0.229` (`derive`) and inherited
serde_json `=1.0.151` (`preserve_order`, `float_roundtrip`) for the embedded brand
projection. These libraries use MIT OR Apache-2.0; they add no runtime library
interface or new resolved package.
