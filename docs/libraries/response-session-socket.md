## Approved response-session socket libraries

Outside browser targets, `maestro-models` uses `tokio-tungstenite =0.30.0` (MIT; defaults
off, `connect` and `rustls-tls-native-roots`) for the authenticated socket upgrade, framing and
TLS with the platform's trust roots. Its closure adds `tungstenite 0.30.0` (MIT OR Apache-2.0),
`sha1 0.11.0` (MIT OR Apache-2.0) and `const-oid 0.10.2` (Apache-2.0 OR MIT); `digest 0.11.3`
gains `alloc` and `oid`. The existing rustls, tokio-rustls, native-certs and aws-lc-rs packages
are reused, and no browser socket library is declared. No library type appears in an interface.
