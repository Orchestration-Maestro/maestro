# Daemon protocol

`maestro-protocol` defines the records shared by daemon clients. It performs no
socket connection, terminal conversion, process launch or workspace mutation.

- `frames` defines client/server messages, rendered frame records, protocol
  version 20 and synchronous length-prefixed binary encoding.
- `request`, `response` and `events` expose the JSON request, result and event
  records. `schema` groups the domain records and their generated JSON Schema.
- `shared` owns agent status references, notification positions, config reload
  status, popup sizes and Windows key records. `Flag` is a transparent boolean
  representation for records with several independent flags.
- `build_info` reads compile-time `MAESTRO_BUILD_CHANNEL` and `MAESTRO_BUILD_ID`;
  absent or blank channel defaults to `stable`.

Binary frames contain a four-byte little-endian payload length followed by the
standard bincode encoding. Readers enforce a caller-supplied maximum before
allocating the payload, reject trailing bytes and distinguish truncated streams
from other I/O errors. The normal frame maximum is 2 MiB; explicit graphics
traffic has a separate 32 MiB constant. Version negotiation accepts version 20
and rejects both older and newer clients, including version zero.

JSON method names, discriminants, field defaults and omission rules belong to
the record definitions. Unknown methods fail deserialization. The committed
[JSON Schema bundle](daemon-protocol.schema.json) is checked against those types.
Pane right-click targets serialize as `maestro` or `pane`; installed plugins
serialize their minimum supported version as `min_maestro_version`. Both records
also accept the original input spelling, documented in their Rust API and the
schema bundle. Compact string enums are preserved; schema descriptions document
these input aliases.

Run the focused tests with `cargo nextest run -p maestro-protocol` and build the
public API documentation with `cargo doc -p maestro-protocol --no-deps`.
