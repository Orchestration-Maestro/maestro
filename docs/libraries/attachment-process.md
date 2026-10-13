## Approved attachment process libraries

Outside browser targets, `maestro-tui` uses Tokio `=1.53.2`, defaults off, with
`process` and `rt` for caller-selected attachment search, whose captured child
comes from `maestro-process`. The tools crate enables Tokio `process`, `time` and
`macros` for explicit command execution on the same captured child, and its native
tests add `test-util`, `net` and `io-util`. Native tests add `net`, `io-util`, `macros` and `rt`
for child readiness sockets, and Unix tests add rustix `=1.1.5`, defaults off, `std` and
`process`, to wait for a direct child's exit without reaping it. Tokio is MIT; no runtime type
enters the completion interface. Native consumer tests use fd `10.5.0`, pinned
with the development tools in `mise.toml`.
