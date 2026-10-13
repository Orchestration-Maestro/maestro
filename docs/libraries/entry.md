# Approved entry libraries

`maestro-entry` uses `serde =1.0.229` with `derive`, `serde_json =1.0.151` with
workspace features (`preserve_order`, `float_roundtrip`), and `indexmap =2.14.2`
with `serde`. All are MIT OR Apache-2.0 and already resolved in the workspace.
Its only workspace dependency is `maestro-request`, for message content and
optional-field records. See [session interchange records](../entry.md).
