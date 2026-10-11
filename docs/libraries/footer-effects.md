## Approved footer-effects libraries

On Unix, `maestro-app` uses Tokio `=1.53.2` (defaults off; `rt`, `time`,
`process`) for asynchronous Git queries and reference-table polling on a
caller-driven `LocalSet`, plus `num-traits =0.2.19` (defaults) for timestamp
conversion without integer-millisecond truncation. Tests add Tokio `rt-multi-thread`
and `sync`. Tokio is MIT and num-traits is MIT OR Apache-2.0. Notifications and
one-shot timers reuse `maestro-watch`; no notification library is duplicated in
the application. Other targets supply `FooterOperations` without these native
process/stat edges.
