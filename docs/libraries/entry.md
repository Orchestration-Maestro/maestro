# Approved entry libraries

The `entry` module of `maestro-request` uses `serde =1.0.229` with `derive`,
`serde_json =1.0.151` with workspace features (`preserve_order`,
`float_roundtrip`), and `indexmap =2.14.2` with `serde`. All are MIT OR Apache-2.0
and already resolved in the workspace. See
[session interchange records](../entry.md).
