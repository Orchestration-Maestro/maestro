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

## Pane topology commands

The daemon answers `pane.list`, `current`, `get`, `focus`, `layout`, `process_info`,
`neighbor`, `edges` and `focus_direction`. A pane target is its stable id, such as `w1:p2`,
or an older spelling such as `w1-2`. `pane.get` and `pane.focus` fail with `pane_not_found`
and `pane <id> not found` when the pane is gone. The other commands take an optional
pane and use the focused pane of the active workspace when none is named; they fail
with `pane_not_found` and `pane not found` when there is no such pane.

- `pane.list` returns the panes of one workspace, or of every workspace, in tab order and
  then split order. An unknown workspace fails with `workspace_not_found`.
- `pane.current` returns the pane named in `caller_pane_id`, which lets a pane ask about
  itself, or the focused pane when none is named.
- `pane.focus` activates the pane's tab and workspace, marks the tab's panes seen and
  returns to terminal input. A pane that already has focus still has its tab marked seen.
- `pane.layout` returns the layout snapshot of the pane's tab: public ids, chrome cells,
  dividers, focus and zoom. `neighbor`, `edges` and `focus_direction` return that
  snapshot with their own answer. `neighbor` names the pane next to the source in a
  direction, `edges` tells which sides of the view area the pane's cell touches, and
  `focus_direction` moves focus to the neighbour, or reports `no_neighbor` and changes
  nothing.
- `pane.process_info` returns the shell's process id and the members of the foreground
  process group with their names, command lines and working directories. A pane without
  a live process fails with `pane_not_found`, and a process observation that fails
  answers `internal_error`.

A created workspace or tab publishes `layout_updated` after its created records.

Every other command fails with `not_implemented` and `method not implemented yet` until
the daemon handles it.

## Pane mutation commands

The daemon also answers `pane.split`, `resize`, `swap`, `zoom`, `move` and `close`. They
take the same pane targets as the topology commands. Each change that alters a tab's
layout publishes `layout_updated` with the tab's new snapshot, and nothing is published
when a command changes nothing.

- `pane.split` splits `target_pane_id`; without one it splits the focused pane of
  `workspace_id`, or of the active workspace. The new pane starts a process in `cwd`, or
  in the directory the configured new-terminal policy picks from the split pane, with
  the `env` pairs added. `direction` is `right` or `down`, `ratio` is the first child's
  share, and `right_click` set to `pane` keeps right clicks for the program in the new
  pane. With `focus` the new pane takes focus, activating its tab. The answer is the
  new pane's record once its process runs, after `pane_created` and `layout_updated`. A
  missing target fails with `pane_not_found`, a bad `env` with `invalid_env`, and a
  process that cannot start with `pane_split_failed`, leaving the tab as it was.
- `pane.resize` moves the divider nearest the pane's edge in `direction` by `amount`,
  which defaults to 0.05 of the split, ignores its sign and is limited to 0.5. Focus does
  not change. When no divider moves the answer has `changed` false and the reason
  `unchanged`.
- `pane.swap` exchanges two panes of one tab: `source_pane_id` with `target_pane_id`, or
  `pane_id` (the focused pane by default) with its neighbour in `direction`. The
  source ends focused. A swap that cannot happen is answered, not refused, with the
  reason `no_neighbor`, `same_pane`, `not_found` or `cross_tab`. Naming both a direction
  and a pair, or neither, or half a pair, fails with `invalid_pane_swap`.
- `pane.zoom` zooms the pane's tab with `mode` `toggle` (the default), `on` or `off`, and
  gives the pane focus. The answer tells apart `zoom_changed` and `focus_changed`; a
  tab of one pane, or a mode that matches the tab, changes no zoom and says why
  (`single_pane`, `already_zoomed`, `already_unzoomed`).
- `pane.move` moves a pane with its terminal: beside a pane of another tab
  (`tab`, with a split direction and ratio), to a new tab of a workspace (`new_tab`) or to
  a new workspace (`new_workspace`). A pane that leaves its workspace gets a handle
  there and keeps answering to the old one; a tab or workspace left empty is closed.
  A move into the pane's own tab, or involving a zoomed tab, is answered with the
  reason `same_tab` or `zoomed_tab` and the layouts concerned. A target pane outside
  the destination tab fails with `target_pane_not_found`. When the destination refuses
  the pane after it was taken, the pane is put back and the command fails with
  `pane_move_failed`. A move publishes `tab_closed` and `workspace_closed` for what it
  emptied, `workspace_created` and `tab_created` for what it made, `pane_moved`, and
  `layout_updated` for the source and the destination tab.
- `pane.close` closes the pane and releases its terminal. The last pane of a tab closes
  the tab, and the last pane of a workspace closes the workspace. It publishes
  `pane_closed`, then `workspace_closed` or `layout_updated`. When closing would close a
  worktree group that asks first, it fails with `confirmation_required` and the daemon
  waits for the answer.

## Pane input and report commands

The daemon also answers `pane.input.set`, `rename`, `read`, `send_text`, `send_keys`,
`send_input`, `report_agent`, `report_agent_session`, `report_metadata`,
`clear_agent_authority` and `release_agent`. Each names its pane; one that is gone fails
with `pane_not_found` and `pane <id> not found`.

- `pane.input.set` sends right clicks in the pane to the program (`pane`) or to Maestro
  (`maestro`).
- `pane.rename` sets the pane's label, or clears it when `label` is missing or blank, and
  answers with the pane. A changed label is published as `pane_updated` with the complete
  record; setting the label it already has publishes nothing.
- `pane.read` returns screen text from `source` (`visible`, `recent`, `recent_unwrapped`
  or `detection`) as `text` or `ansi`. `lines`, at most 1000, keeps the last lines of a
  `visible` or `detection` read and sets how many rows a `recent` read takes (80 without
  it). The answer names the pane, its workspace and its tab, and `truncated` says that
  older lines or rows were left out. A pane without a live process fails with
  `pane_not_found`.
- `pane.send_text` writes `text` as given. `pane.send_keys` writes each key as its own
  write. `pane.send_input` writes `text`, framed as a paste when the program asked for
  bracketed paste, and then the keys, as one write. Keys follow the key-binding grammar
  (`ctrl+c`, `shift+tab`, `Enter`), and `C-c`, `c-c` and a lone `+` are also accepted.
  Every key is checked before a byte is written. A key that is not one, including a
  prefix binding such as `prefix+h`, fails with `invalid_key` and `unsupported key <key>`.
  A pane whose process cannot take the bytes fails with `pane_send_failed`.
- `pane.report_agent` sets the state an integration reports (`idle`, `working`, `blocked`
  or `unknown`) for `agent` from `source`, with an optional `message`, sequence and session
  identity. `pane.report_agent_session` records the session identity alone.
  `pane.clear_agent_authority` withdraws the authority of `source`, or of any source, and
  the detected state shows again. `pane.release_agent` hands the pane back to the shell;
  an official integration's release waits for its live process to exit. A blank `agent`
  fails with `invalid_agent`. A recognized agent is stored under its catalogue name, and
  any other label is kept as it is.
- `pane.report_metadata` sets a title, display agent, state labels and tokens for a
  `source`, with a lifetime of 1 ms to one day. Text is cleaned of control characters and
  limited to 80 characters, and a state label must name an execution state. A request
  that sets and clears one field, or asks for nothing, fails with
  `invalid_metadata_request`. A pane holds at most 32 tokens, a request sets at most 16,
  and sequences are tracked for at most 32 token sources. Presentation and tokens share
  one sequence per source; a stale sequence, or a report that follows the exit of the
  agent it names, is accepted without effect. A changed token set publishes `pane_updated`.
  The other failures are `invalid_metadata_source`, `invalid_metadata_token`,
  `invalid_metadata_ttl`, `invalid_state_label`, `metadata_token_limit` and
  `metadata_sequence_source_limit`.

## Layout commands

The daemon also answers `layout.export`, `layout.apply` and `layout.set_split_ratio`. A
layout is a tree: a split has a `direction` (`right` or `down`), the first child's `ratio`
and two children; a pane leaf has its `pane_id`, `label`, `cwd`, `command` (a program and
its arguments, run without a shell) and `env`. An answer carries the workspace, the tab,
`zoomed`, the focused pane and the tree.

- `layout.export` describes the tab named by `tab_id`, the tab holding `pane_id`, or the
  active tab when neither is named. Naming both, or a tab or pane that is gone, fails with
  `layout_not_found` and `layout target not found`. A pane's `command` is the one it was
  launched with, and its `cwd` is the live directory when its process runs.
- `layout.set_split_ratio` sets the ratio of the split reached by `path`, which lists `false`
  for the first child and `true` for the second from the root. The ratio is kept between
  0.1 and 0.9. The answer is the tab's layout, after `layout_updated`. A ratio that is not
  finite fails with `invalid_ratio`, and a path that reaches no split with
  `split_not_found`; nothing is published then.
- `layout.apply` builds `root` as a new tab, starting one process per leaf. It goes to
  `workspace_id`, to the workspace of `tab_id`, or to the active workspace, and with
  `tab_id` the new tab replaces that tab once it stands. Naming both fails with
  `invalid_target`; a missing tab or workspace fails with `tab_not_found` or
  `workspace_not_found`. The first leaf starts in its `cwd`, else in the directory the
  configured new-terminal policy picks from the replaced tab's focused pane or the
  workspace's focused pane; any other leaf starts in its `cwd`, else where the pane it
  splits runs. `tab_label` names the tab; a replacing tab keeps the name of the tab it
  replaces when none is given. The new tab becomes active with `focus`, or when it replaces
  the active tab. A tree of more than 24 panes or 16 levels, an empty `command`, a bad
  `env` or a ratio that is not finite fails with `invalid_layout` before anything starts.
  A process that cannot start fails with `layout_apply_failed`, and the tab built so far
  is removed again, leaving the replaced tab as it was. A success publishes `tab_closed`
  for a replaced tab, `tab_created`, `pane_created` for each pane in layout order, and
  `layout_updated`.

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
