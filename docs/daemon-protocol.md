# Daemon protocol

`maestro-protocol` owns Maestro's shared RPC records and binary screen channel.
It performs no socket connection, terminal conversion, process launch or workspace
mutation. The [foundation contract](specs/maestro-port.md#workspace-daemon) defines
the single command set served on stdio, daemon sockets and SDK connections.

## Commands and responses

`rpc_types::Request` has an optional string `id`, a `type` command discriminator
and typed parameter fields at the top level, for example:

```json
{
  "id": "w1",
  "type": "workspace.create",
  "cwd": "/workspace",
  "focus": true
}
```

`RpcCommand` contains the 92 workspace commands, and `Request` defaults to it. The
application composes them with the 29 session commands under the same `type` tag in
`maestro_app::rpc_types::RpcCommand`, which also owns the session responses, session
events and extension dialog records; see the [JSONL protocol](rpc.md). Both
response families write their envelope through `rpc_types::serialize_rpc_response`.
There is no second command registry here. Frontends consume contracts through
application re-exports.

`RpcResponse` carries the original `command` and optional correlation `id`,
`type: "response"`, and a boolean `success`. A successful workspace operation
has typed `data`. A workspace failure has a string `error` and an open string
`code`, preserving machine-readable failure kinds without parsing prose. Session
failures keep their established shape without a workspace `code` field.
Old `method`/`params` requests and result/error-object envelopes are not supported.
Unknown command types and malformed known parameters fail deserialization;
unknown members on domain records are ignored.

Action commands are `action.manifest.link`, `action.manifest.list`,
`action.manifest.unlink`, `action.manifest.enable`, `action.manifest.disable`,
`action.list`, `action.invoke`, `action.log.list`, `action.pane.open`,
`action.pane.focus` and `action.pane.close`. There are no old command aliases.
`ActionManifestInfo` uses `manifest_id` and `manifest_root`. The minimum-version
field emits `min_maestro_version` and accepts `min_herdr_version` on input.
Pane right-click targets emit `maestro` or `pane` and accept the original target
spelling as an input alias; their Rust API documents these aliases.

`PackageSourceInfo` projects local, Git and npm package sources with their settings
`source` string and optional `subdir`, `requested_ref`, `resolved_commit`,
`managed_path` and `installed_unix_ms`. The timestamp describes installation of
the current checkout; producers leave it absent for local sources. Link accepts
only `path` and `enabled` (default true); obsolete source input is ignored.

## Events and version

`EventData` is the top-level `type`-tagged workspace event family. The 26 ordinary
event payloads retain their snake-case tags. Parameterized output-match and
scroll notifications use `pane_output_matched` and `pane_scroll_changed`; status
notifications use the existing `pane_agent_status_changed` payload. There is no
outer `event`/`data` wrapper. The 27 subscription selectors retain dotted names,
filters, replay inputs, match options and wait results containing that same event
family. Runtime delivery belongs to the daemon execution layer; a plain session
client is not automatically subscribed to workspace events.

`maestro_protocol::PROTOCOL_VERSION` is 21. Metadata, snapshots, handshakes and
handoff use this one protocol version; `frames` re-exports the same constant.
The generated schema's `schema_version: 1` is its document format, not another
negotiated protocol. Product versions remain separate strings. Existing JSONL
clients need no header, handshake, version field or unsolicited startup record.
JSONL framing belongs to `maestro-json::jsonl`, not a second codec here.

## Socket server

`maestro_rpc::daemon::run_server` serves this protocol on the daemon's API socket.
Each accepted connection runs the application's one connection runner without a
session, so a line read during a long request is the next command, and responses
can arrive out of order. Match them by `id`. A connection ends when the client ends
its input: requests still waiting on the daemon get no response, but a request the
daemon already took is not undone.

- The first line must arrive within 5 s of the connection's acceptance. A partial
  line does not extend the deadline. Once the first line arrives, later lines have
  no deadline.
- A line is at most 1 MiB before its LF, a CR included, and must be valid UTF-8.
  A longer line or invalid UTF-8 closes the connection without a response.
- Each line is trimmed of Unicode whitespace before it is parsed; U+FEFF is not
  whitespace. A blank first line, or input that ends before a complete first line,
  closes the connection without a response.
- `ping` answers at once, even while the daemon is busy. It reports the product
  version, protocol 21 and the server's capabilities.
- `server.stop` answers at once and stops the daemon. Once stopped, every request
  but `ping` fails with `server_unavailable` and `server is shutting down`.
- Other requests wait for the daemon however long it takes. A daemon that is gone
  fails them with `server_unavailable`.
- A response write that makes no progress for 5 s fails the connection. Each write
  that takes bytes starts a new budget. The daemon learns that a response was
  written once its whole line was written and flushed, or once that attempt failed.
  A client that closed its end is not a write failure.

Commands without a daemon handler yet fail with `not_implemented` and
`method not implemented yet`.

## Binary screen channel

Binary is explicitly selected at connection creation, using `Hello`/`Welcome` on
that connection. There is no in-band switch between JSONL and binary. Detach or
error closes the binary connection; it never returns to JSONL. This rule applies
to local attach and SSH stdio, graphics streams and live handoff. Ordinary JSONL
streams never receive raw frames or image bytes. Execution integration is owned
by the screen and remote-client deliveries.

`frames` retains semantic cells, ANSI output, graphics, clipboard, input, resize,
attach, observe, control, takeover, detach and handoff records. Frames contain a
four-byte little-endian payload length followed by standard bincode encoding.
Readers enforce the caller-supplied maximum before allocation, reject trailing
bytes and distinguish truncated streams from other I/O errors. The normal maximum
is 2 MiB; explicitly selected graphics traffic has a separate 32 MiB constant.
Decoder accounting is limited to eight times the received payload length rounded
up to a power of two (at least one byte), saturating at the platform's addressable
size. This allows compact integer encodings while rejecting excessive declared
string lengths before allocation. The limit measures decoder accounting, not total
heap usage. Serde caps each collection's speculative preallocation at 1 MiB;
total storage for valid records can exceed the accounting.
Version negotiation accepts 21 and rejects older/newer clients, including zero.

`ClientKeybindings::Server` keeps server configuration. `Local` carries a typed
`bindings` map from action identifiers to string lists, including `prefix`;
an empty list disables an action. This map decodes through the same binary codec,
without TOML text or self-describing JSON values.

## Schema and checks

The committed [JSON Schema bundle](daemon-protocol.schema.json) is checked against
the delivered records. The RPC frontend delivery extends this same bundle for
session and UI traffic. Compact string enums and documented input aliases remain
part of the schema.

Run `cargo nextest run -p maestro-protocol` for wire/codec tests and
`cargo doc -p maestro-protocol --no-deps` for public API documentation.
