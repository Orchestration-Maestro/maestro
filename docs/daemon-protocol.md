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
`code`, so a client reads the failure kind without parsing prose. Session
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
- `server.stop` answers at once and stops the daemon after its response write
  attempt settles. Once stopping, every request but `ping` and `server.stop` fails
  with `server_unavailable` and `server is shutting down`.
- Other requests wait for the daemon however long it takes. A daemon that is gone
  fails them with `server_unavailable`.
- A response write that makes no progress for 5 s fails the connection. Each write
  that takes bytes starts a new budget. The daemon learns that a response was
  written once its whole line was written and flushed, or once that attempt failed.
  A client that closed its end ends the connection normally, but the daemon's
  private write receipt still reports the failed write attempt.

## Workspace and tab commands

The daemon answers the workspace commands `workspace.list`, `get`, `create`, `focus`,
`rename`, `move`, `move_block`, `report_metadata` and `close`, and the tab commands
`tab.list`, `get`, `create`, `focus`, `rename`, `move` and `close`. A workspace target
is its stable id, such as `w1`, or a position written `w_N` or `N`. A tab target is
`w1:t2`, or a position written `w1:2` or `t_w1_2`. A target that names nothing fails
with `workspace_not_found` or `tab_not_found`.

- `workspace.create` and `tab.create` start the new root pane in `cwd`. Without one,
  the pane starts where the `newPaneCwd` setting points, which by default is the
  focused pane's directory. The result holds the new workspace or tab and its root
  pane. With `focus: false` the existing active workspace and tab stay active.
  Workspace creation activates the new workspace if none was active.
- `env` gives the new root pane extra variables. A key must be non-empty and hold no
  `=` or NUL, and a value must hold no NUL. Otherwise the command fails with
  `invalid_env` before any pane starts.
- A move takes an insertion index from 0 to the number of workspaces or tabs.
  `workspace.move_block` moves its workspaces, in the order given, before
  `before_workspace_id` or to the end. A move that changes nothing publishes no event.
- Closing a workspace, or its last tab, publishes the workspace's final record. While
  close confirmation is on, closing the last tab of a repository with open linked
  worktrees fails with `confirmation_required`.

`workspace.report_metadata` sets or clears at most 16 tokens per report, and a
workspace holds at most 32. A key is at most 32 ASCII letters, digits, `_` or `-`. A
value is trimmed, loses its control characters and keeps at most 80 characters; a
blank value clears its token. `source` names the reporter in at most 80 ASCII letters,
digits, `:`, `.`, `_` or `-`. `ttl_ms`, from 1 to 86400000, makes the tokens expire.
With `seq`, a report no newer than the reporter's last one has no effect, and at most
32 reporters can use `seq` on one workspace.

`notification.show` shows a workspace notification when notifications go to the
workspace. CR, LF and tab become spaces; other control characters go. Whitespace
runs become one space. The title keeps at most 80 characters and the body at most
240. An empty title fails with
`invalid_params`. The result reason is `busy` while another notification shows,
`rate_limited` within a second of the last one, and `disabled` when notifications are
off. Terminal and system notifications need an attached client, so the daemon answers
`no_foreground_client`.

`server.reload_config` reloads the settings and reports what applied.
`server.agent_manifests` reports the detection manifests in use and the last catalog
check. `server.reload_agent_manifests` reloads them and restarts detection in every
pane.

The other command families fail with `not_implemented` and
`method not implemented yet` until the daemon handles them.

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
