# Tool output libraries

`maestro-tools` uses workspace-pinned `serde =1.0.229` with derive and default
`std` for output serialization. Records use camelCase field names; reasons use
lowercase strings. No output record is deserialized by the product.

Workspace-pinned `ryu-js =1.0.3` supplies number spelling and one-decimal fixed
formatting for [size text](../tools-read.md#line-clipping-and-size-text). Use its
Apache-2.0 license option. Serde and its derive closure have permitted MIT or
Apache-2.0 options; unicode-ident retains Unicode-3.0.

Tests use workspace `serde_json =1.0.151` with `preserve_order` and
`float_roundtrip` to compare independently recorded output trees.

Native [streaming output](../tools-read.md#native-streaming) additionally uses
`maestro-path` for lexical filename joining and `getrandom =0.4.3`, with no extra
features, for artifact identifiers. Its native dependency closure uses existing
`cfg-if =1.0.5`, `libc =0.2.190` and target-specific `r-efi =6.0.0`; use their
MIT or Apache-2.0 license options. Files and buffered writes use the standard
library. The streaming module is excluded from browser builds.
