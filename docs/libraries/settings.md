## Approved settings libraries

The settings crate uses serde `=1.0.229` with `derive`, serde_json `=1.0.151`
with `preserve_order` and `float_roundtrip` (exact decimal parsing, enabled once
on the workspace dependency for every crate), Tokio `=1.53.2` with defaults off
and `sync` for completion barriers, and `ryu-js =1.0.3` for float spelling at
the JSON boundary (Apache-2.0 OR BSL-1.0).
Browser scheduling uses wasm-bindgen-futures `=0.4.79`. Native tests add Tokio's
`rt` feature. These add no internal crate edge or public runtime API.
