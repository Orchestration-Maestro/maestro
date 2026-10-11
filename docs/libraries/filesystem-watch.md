## Approved filesystem-watch libraries

Outside browser targets, `maestro-watch` uses `notify =8.2.0` with defaults (CC0-1.0) for
non-recursive directory notifications and Tokio `=1.53.2`, defaults off, with `rt`, `time`
and `sync` for the caller-driven local task set, timer and notification channel.
The added closure is `notify-types 2.1.0`, `inotify 0.11.5` and `inotify-sys 0.1.8` (ISC),
`mio 1.2.4`, `bitflags 2.13.2`, `log 0.4.34`, `pin-project-lite 0.2.17`, `same-file 1.0.6`
and `walkdir 2.5.0` (MIT OR Apache-2.0 or MIT), plus `fsevent-sys 4.1.0`, `kqueue 1.2.1` and
`kqueue-sys 1.1.2` on the platforms that use them. No library type appears in a public
interface except Tokio's `LocalSet`, which the caller supplies to
`NativeWatchOperations::new`. Browser callers supply their own `WatchOperations`.
Native watch tests add Tokio `test-util`; theme retains native-only dev Tokio
`rt`, `time` and `sync` for its integration tests.
