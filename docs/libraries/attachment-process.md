## Approved attachment process libraries

Outside browser targets, `maestro-tui` uses Tokio `=1.53.2`, defaults off, with
`process`, `io-util`, `rt` and `macros` for caller-selected attachment search.
Native tests add `net` for child readiness sockets. Tokio is MIT; no runtime type
enters the completion interface. Native consumer tests use fd `10.5.0`, pinned
with the development tools in `mise.toml`.
