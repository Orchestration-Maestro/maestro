## Approved response-session socket libraries

Outside browser targets, `maestro-models` uses `tokio-tungstenite =0.30.0` (MIT; defaults
off, `connect` and `rustls-tls-native-roots`) for the authenticated socket upgrade, framing and
TLS with the platform's trust roots. Its closure adds `tungstenite 0.30.0` (MIT OR Apache-2.0),
`sha1 0.11.0` (MIT OR Apache-2.0) and `const-oid 0.10.2` (Apache-2.0 OR MIT); `digest 0.11.3`
gains `alloc` and `oid`. The existing rustls, tokio-rustls, native-certs and aws-lc-rs packages
are reused. The native adapter in
[`providers::websocket`](../../crates/maestro-models/src/providers/websocket/mod.rs)
owns the single upgrade primitive used by response sessions and model discovery.
The application uses the same pinned library for its native test peers. No
library type appears in the public socket-effect interface.

The browser adapter uses the existing `web-sys =0.3.106` bindings with `WebSocket`,
`BinaryType`, `CloseEvent` and `ErrorEvent` enabled, alongside existing event APIs.
It retains registrations through the browser's event-subscription owner, with weak
queue captures and synchronous release. No additional browser socket library,
retry loop or worker is introduced.
