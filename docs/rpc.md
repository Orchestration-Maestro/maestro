# JSONL protocol

Maestro can run headless and take commands as JSON lines on standard input. It answers
and streams events as JSON lines on standard output. Embedders drive a full session this
way: prompts, models, queues, compaction, shell commands, session files and extension
dialogs.

The protocol has one connection runner, `maestro_app::run_rpc_connection`. It serves a
local session runtime over a caller's line reader and writer and returns how the
connection ended; it never exits the process. `maestro_rpc::run_rpc_mode` is its
standard-stream adapter: it reserves standard output for protocol records, serves the
runtime and exits with the connection's code.

## Framing

- Each record is one JSON object followed by one LF. Input lines may end in CRLF; one CR
  before the LF is removed, and nothing else.
- A final line without its LF is read when the input ends.
- Line and paragraph separators (U+2028, U+2029) are ordinary characters inside a record
  and never split one. Output writes them unescaped.
- Output is compact JSON. Managed output that is not a protocol record goes to standard
  error while the server runs.
- There is no startup record, handshake, header or version member on this stream.

## Commands and responses

A command is an object with a `type` and an optional string `id`. Every command gets one
response:

```json
{"id": "a", "type": "response", "command": "get_state", "success": true, "data": {}}
{"id": "a", "type": "response", "command": "set_model", "success": false, "error": "Model not found: p/m"}
```

`id` is echoed when the command had a valid one. `data` is omitted when a command answers
no data, and `null` when it answers null. Commands run concurrently: a slow command does
not hold back a fast one, so responses can arrive out of order. Match them by `id`.

A record that is not valid JSON, not an object, or has no string `type` is answered with
an uncorrelated failure. Invalid JSON uses the command `parse` and the message
`Failed to parse command: ` followed by the decoder's message. An unknown `type` answers
`Unknown command: <type>` without the supplied `id`. A known command with a missing or
wrongly typed field keeps its `id` and command and answers the decoder's message. The
connection keeps serving after each of these.

### Session commands

| Command | Fields | Data on success |
|---|---|---|
| `prompt` | `message`, optional `images`, optional `streamingBehavior` (`steer` or `followUp`) | none |
| `steer` | `message`, optional `images` | none |
| `follow_up` | `message`, optional `images` | none |
| `abort` | | none, sent once the agent is idle |
| `new_session` | optional `parentSession` | `{cancelled}` |
| `get_state` | | session state |
| `set_model` | `provider`, `modelId` | the model |
| `cycle_model` | | `{model, thinkingLevel, isScoped}` or `null` |
| `get_available_models` | | `{models}` |
| `set_thinking_level` | `level` | none |
| `cycle_thinking_level` | | `{level}` or `null` |
| `set_steering_mode` | `mode` (`all` or `one-at-a-time`) | none |
| `set_follow_up_mode` | `mode` (`all` or `one-at-a-time`) | none |
| `compact` | optional `customInstructions` | the compaction result |
| `set_auto_compaction` | `enabled` | none |
| `set_auto_retry` | `enabled` | none |
| `abort_retry` | | none |
| `bash` | `command` | the shell result |
| `abort_bash` | | none |
| `get_session_stats` | | session statistics |
| `export_html` | optional `outputPath` | `{path}` |
| `switch_session` | `sessionPath` | `{cancelled}` |
| `fork` | `entryId` | `{text, cancelled}`; `text` is omitted when cancelled |
| `clone` | | `{cancelled}` |
| `get_fork_messages` | | `{messages}`, each `{entryId, text}` |
| `get_last_assistant_text` | | `{text}`; `text` is omitted without an assistant message |
| `set_session_name` | `name` | none |
| `get_messages` | | `{messages}` |
| `get_commands` | | `{commands}` |

A prompt is answered once, as soon as it is accepted: after its preflight succeeds, when
an input handler handles it, or when it is queued behind a running turn. A failure after
acceptance produces no second response; the turn's events report it. A preflight failure
is the one response.

`set_session_name` trims leading and trailing whitespace, U+FEFF included and U+0085 and
U+200B kept, and refuses an empty result with `Session name cannot be empty`. `clone`
without a current entry fails with `Cannot clone session: no current entry selected`.

The session state has `model`, `thinkingLevel`, `isStreaming`, `isCompacting`,
`steeringMode`, `followUpMode`, `sessionFile`, `sessionId`, `sessionName`,
`autoCompactionEnabled`, `messageCount` and `pendingMessageCount`. `model`, `sessionFile`
and `sessionName` are omitted when absent.

Each entry of `get_commands` has `name`, an optional `description`, `source` (`extension`,
`prompt` or `skill`) and `sourceInfo`, the provenance of the extension, template or skill
that defines it. Extension commands come first, then prompt templates, then skills, in
their registration order.

A successful `new_session`, `switch_session`, `fork` or `clone` binds the replacement
session once: its extensions start once and its events reach the client once. A
replacement an extension cancels binds nothing.

### Workspace commands

Workspace commands use the same `type` tag and envelopes; see the
[daemon protocol](daemon-protocol.md) for each command. They reach the workspace
operations the embedder supplies. A workspace failure also carries its `code`:

```json
{"id": "g", "type": "response", "command": "workspace.get", "success": false, "error": "no workspace w9", "code": "workspace_not_found"}
```

Workspace events are written only after a client subscribes, in the same type-tagged
format. Live screen traffic stays in the binary channel a client selects explicitly; this
stream never carries binary bytes.

## Events

The server streams these session events as they happen, without an `id`:

| Event | Members |
|---|---|
| `agent_start` | |
| `agent_end` | `messages` |
| `turn_start` | |
| `turn_end` | `message`, `toolResults` |
| `message_start` | `message` |
| `message_update` | `message`, `assistantMessageEvent` |
| `message_end` | `message` |
| `tool_execution_start` | `toolCallId`, `toolName`, `args` |
| `tool_execution_update` | `toolCallId`, `toolName`, `args`, `partialResult` |
| `tool_execution_end` | `toolCallId`, `toolName`, `result`, `isError` |
| `queue_update` | `steering`, `followUp` |
| `compaction_start` | `reason` |
| `compaction_end` | `reason`, `result`, `aborted`, `willRetry`, optional `errorMessage` |
| `auto_retry_start` | `attempt`, `maxAttempts`, `delayMs`, `errorMessage` |
| `auto_retry_end` | `success`, `attempt`, optional `finalError` |
| `session_info_changed` | optional `name` |
| `thinking_level_changed` | `level` |
| `extension_error` | `extensionPath`, `event`, `error` |

Each event carries the payload it had when it was emitted, so a streamed partial message is
the partial of that moment. `extension_error` reports an extension's failure and is never
correlated with a command.

## Extension dialogs

Extensions ask questions through `extension_ui_request` records, each with a fresh UUID v4
`id` and a `method`:

| Method | Members | Answered |
|---|---|---|
| `select` | `title`, `options`, optional `timeout` | yes |
| `confirm` | `title`, `message`, optional `timeout` | yes |
| `input` | `title`, optional `placeholder`, optional `timeout` | yes |
| `editor` | `title`, optional `prefill` | yes |
| `notify` | `message`, optional `notifyType` | no |
| `setStatus` | `statusKey`, optional `statusText` | no |
| `setWidget` | `widgetKey`, optional `widgetLines`, optional `widgetPlacement` | no |
| `setTitle` | `title` | no |
| `set_editor_text` | `text` | no |

The client answers a dialog with an `extension_ui_response` naming its `id`: `value` for
`select`, `input` and `editor`, `confirmed` for `confirm`, or `cancelled: true` for any of
them. A dismissal wins over other members. A present member of the wrong kind, `null`
included, fails the extension's dialog and gets no reply. A response to an unknown,
settled or malformed `id` is ignored.

A dialog with a positive `timeout` settles with its default after that many milliseconds; a
zero or absent timeout waits. An `editor` has no timeout. A dialog whose signal already
aborted writes nothing and returns its default.

Dialogs work while the first session is still binding: a `session_start` handler can ask
one, and its answer finishes the binding. Other commands that arrive meanwhile wait and
then run in arrival order.

Widget factories, custom components, footers, headers, editor components and
autocomplete providers need a terminal: they are never built here. Theme selection
answers `Theme switching not supported in RPC mode`.

## Ending

The connection ends when the input ends, when an extension requests shutdown, or on SIGTERM
or SIGHUP. At the end of input the server stops streaming events, starts disposing the
session and answers a final line without its LF; the process exits 0. A shutdown an
extension requests takes effect after the next command's response. The first signal kills
the shell commands still running, disposes the session and exits with 143 or 129. Once
shutdown has started, a second signal kills the process by that signal, and the end of
input exits 0 at once.
