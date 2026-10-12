## Approved attachment process libraries

Outside browser targets, `maestro-tui` uses Tokio `=1.53.2`, defaults off, with
`process` and `rt` for caller-selected attachment search, whose captured child
comes from `maestro-process`. Native tests add `net`, `io-util`, `macros` and `rt`
for child readiness sockets. Tokio is MIT; no runtime type
enters the completion interface. Native consumer tests use fd `10.5.0`, pinned
with the development tools in `mise.toml`.
