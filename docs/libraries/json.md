# Approved JSON libraries

`maestro-json` uses `serde =1.0.229` with `derive`, `serde_json =1.0.151` with the
workspace features (`preserve_order`, `float_roundtrip`) plus `raw_value`, and
`ryu-js =1.0.3` with defaults for ECMAScript number spelling. serde and serde_json
are MIT OR Apache-2.0; ryu-js is Apache-2.0 OR BSL-1.0, used under Apache-2.0. The leaf
adds no workspace edge and no package that the workspace did not already resolve.
`maestro-settings` no longer declares `ryu-js` because saved text comes from this
leaf. See [JSON](../json.md).
