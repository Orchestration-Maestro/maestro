## Approved settings libraries

The settings crate uses serde `=1.0.229` with `derive`, serde_json `=1.0.151`
with `preserve_order` and `float_roundtrip` (exact decimal parsing, enabled once
on the workspace dependency for every crate), Tokio `=1.53.2` with defaults off
and `sync` for completion barriers. Saved text and its float spelling come from
`maestro-json`, which owns `ryu-js =1.0.3` ([JSON libraries](json.md)).
Browser scheduling uses wasm-bindgen-futures `=0.4.79`. Native tests add Tokio's
`rt` feature. These add no internal crate edge or public runtime API.
