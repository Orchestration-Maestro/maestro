# Daemon protocol libraries

`maestro-protocol` uses `bincode =2.0.1` (MIT), defaults disabled with `serde`
and `std`, for binary messages. It uses `schemars =1.2.3` (MIT), with its default
`derive` and `std` features, to generate JSON Schema from the protocol records.
Both operate on the existing workspace serde and serde_json records.

The only domain dependency is `maestro-json`; the native feature unifier follows
the workspace convention. No terminal, input, socket or daemon runtime dependency
is introduced. See the [daemon protocol](../daemon-protocol.md).
