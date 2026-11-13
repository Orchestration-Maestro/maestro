# Approved JSON libraries

`maestro-json` uses `serde =1.0.229` with `derive`, `serde_json =1.0.151` with the
workspace features (`preserve_order`, `float_roundtrip`) plus `raw_value`, and
`ryu-js =1.0.3` with defaults for ECMAScript number spelling. Raw-value reading uses
`indexmap =2.14.2` with defaults and `serde` for object members in source order, and
`num-traits =0.2.19` with defaults for exact integer storage of whole numbers. serde,
serde_json, indexmap and num-traits are MIT OR Apache-2.0; ryu-js is Apache-2.0 OR
BSL-1.0, used under Apache-2.0. The leaf adds no workspace edge and no package that
the workspace did not already resolve.
`maestro-settings` uses the same pinned `ryu-js` to spell rejected numeric idle
timeouts, including `NaN`, `Infinity` and `-Infinity`. Saved JSON text comes from
this leaf. See [JSON](../json.md).
