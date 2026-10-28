# RPC integration guide

Maestro can run headless and take commands as JSON lines on standard input. It
answers and streams events as JSON lines on standard output. Use it to send
prompts, select models, manage queues, compact context, run shell commands,
replace sessions and respond to extension dialogs.

The protocol has one connection runner, `maestro_app::run_rpc_connection`. It serves an
optional workspace command handler and an optional local session over a caller's line
reader and writer and returns how the connection ended; it never exits the process.
`RpcModeOptions::workspace` supplies the workspace handler. With `None`, workspace
commands receive the ordinary unknown-command response, as they do in the session-only
executable. `RpcModeOptions::session` groups the runtime, signal registration, theme and
export facilities. Without a session, a supplied workspace handler still runs and session
commands return a correlated refusal asking the caller to open in a session.

The writer receives each record as an `RpcLine`, already framed. An `RpcOutput` built with
`RpcOutput::synchronous` writes each line before the write call returns, and
`rpc_mode::stdout_output` is that output on standard output. `maestro_rpc::run_rpc_mode` is its
standard-stream adapter: it reserves standard output for protocol records, serves the
runtime and exits with the connection's code.

`RpcOutput::new` takes an asynchronous writer instead. Lines queue in the order the connection
admits them, and one task writes them. With a stall timeout, a write or flush that makes no
progress for that long fails the connection. Each write that takes bytes starts a new budget,
so a slow reader that keeps reading is not cut off. A workspace operation receives a
write-completion receiver. It settles once the whole response line was written and flushed, or
once that attempt failed. It disconnects if the response is dropped before any attempt, and it
says nothing about whether the peer read the line. After a write fails, the connection admits
no further lines, so no frame follows a partial one.

When the input of a connection without a session ends, the workspace operations still waiting
are dropped and get no response. Responses already queued are still written, and a write that
fails after the connection ended becomes its result. Commands still running when the runner
returns, or when the caller drops it, are dropped with it.

## Start a connection

Launch `maestro --mode rpc` as a child with separate stdin, stdout and stderr
pipes. Keep stdin open until the work finishes. Closing it immediately after
writing a prompt ends the connection, not merely the request.

| Option | Use |
|---|---|
| `--provider NAME --model ID` | Select an available model explicitly. |
| `--no-session` | Keep the session in memory instead of saving it. |
| `--session-dir PATH` | Choose a directory for saved sessions and lookup. |
| `--session PATH` | Resume a saved session. |
| `--no-tools` | Disable model-invoked tools; this does not disable the RPC `bash` command. |
| `--no-extensions --no-skills --no-prompt-templates --no-context-files` | Disable discovered extension and instruction resources for an isolated run. |
| `--offline` | Disable startup network operations, not model requests. |

Without `--no-session`, Maestro saves session history. Session and resource
paths are relative to the child's working directory where applicable. See
[command-line startup](cli.md), [model selection](selection.md) and
[session storage](sessions/session_manager.md) for configuration and lookup rules.
RPC does not sandbox tools or shell commands. Run the child with the filesystem
permissions you intend to grant it; do not pass untrusted shell text to `bash`.

The [Python subprocess example](../examples/sdk/rpc.py) uses only Python's standard
library. From the repository root, inspect a session without making a model call:

```sh
python3 examples/sdk/rpc.py -- maestro --mode rpc --no-session --no-tools --offline --no-extensions --no-skills --no-prompt-templates --no-context-files
```

It sends `get_state`, prints incoming records, cancels extension dialogs and closes
stdin after the correlated response. To run a prompt, add `--prompt TEXT` before
`--`; that operation can incur provider charges. Select your configured provider
and model in the child arguments. The example waits for both prompt acceptance
and `agent_end`, not just the successful response. It leaves stderr separate and
checks the child's exit status. Use plain prompt text: an extension slash
command handled without a run does not produce `agent_end`. The example does
not track automatic retries or overflow recovery after the first run. Use the
[typed Rust client](client.md) for reusable subprocess operations.

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

## Drive a session

Each record below occupies one wire line. Example paths, session IDs, timestamps
and provider URLs are illustrative; obtain real values from responses. The
`fixture/controlled` model is a controlled local provider, not a built-in catalog
entry. Use a pair returned by `get_available_models` in your integration.

Model, thinking, queue-mode and automatic-compaction/retry commands can update
saved settings even with `--no-session`. Rehearse them with a disposable
`MAESTRO_CODING_AGENT_DIR`; see [settings storage](sessions/settings.md).

### Inspect before sending work

```json
{"id":"state","type":"get_state"}
{"id":"state","type":"response","command":"get_state","success":true,"data":{"model":{"id":"controlled","name":"Controlled","api":"openai-completions","provider":"fixture","baseUrl":"http://127.0.0.1:9000/v1","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128000,"maxTokens":64},"thinkingLevel":"off","isStreaming":false,"isCompacting":false,"steeringMode":"one-at-a-time","followUpMode":"one-at-a-time","sessionId":"session-a","autoCompactionEnabled":true,"messageCount":0,"pendingMessageCount":0}}
{"id":"messages","type":"get_messages"}
{"id":"messages","type":"response","command":"get_messages","success":true,"data":{"messages":[]}}
{"id":"last","type":"get_last_assistant_text"}
{"id":"last","type":"response","command":"get_last_assistant_text","success":true,"data":{}}
{"id":"commands","type":"get_commands"}
{"id":"commands","type":"response","command":"get_commands","success":true,"data":{"commands":[]}}
```

In memory, state omits `sessionFile`. A saved session includes that path;
`sessionName` appears after naming it. Treat the booleans as observations, not
locks: another command can change the state before your next request.

`get_messages` returns the current conversation, not the complete persisted tree.
Use `get_fork_messages` for entry IDs; messages themselves do not carry entry IDs.
`get_last_assistant_text` answers `{}` before any assistant message, not
`{"text":null}`. After a text answer it returns `{"text":"Checked"}`.

An invocable command can look like this:

```json
{"name":"skill:review","description":"Review a patch","source":"skill","sourceInfo":{"source":"local","scope":"project","origin":"top-level","path":"/work/project/.maestro/skills/review/SKILL.md"}}
```

`sourceInfo` contains `path`, `source`, `scope` and `origin`, with optional
`baseDir`; see [resource provenance](application/resources.md). Prompt it as
`/skill:review`. The list does not include terminal-only built-in commands.

### Select a model and its thinking level

```json
{"id":"models","type":"get_available_models"}
{"id":"models","type":"response","command":"get_available_models","success":true,"data":{"models":[{"id":"controlled","name":"Controlled","api":"openai-completions","provider":"fixture","baseUrl":"http://127.0.0.1:9000/v1","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128000,"maxTokens":64}]}}
{"id":"model","type":"set_model","provider":"fixture","modelId":"controlled"}
{"id":"model","type":"response","command":"set_model","success":true,"data":{"id":"controlled","name":"Controlled","api":"openai-completions","provider":"fixture","baseUrl":"http://127.0.0.1:9000/v1","reasoning":false,"input":["text"],"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},"contextWindow":128000,"maxTokens":64}}
{"id":"cycle","type":"cycle_model"}
{"id":"cycle","type":"response","command":"cycle_model","success":true,"data":null}
{"id":"thinking","type":"set_thinking_level","level":"high"}
{"id":"thinking","type":"response","command":"set_thinking_level","success":true}
{"id":"next-thinking","type":"cycle_thinking_level"}
{"id":"next-thinking","type":"response","command":"cycle_thinking_level","success":true,"data":null}
```

A model includes its endpoint, API, provider, supported input kinds, reasoning
support, context/output limits and prices per million tokens. Optional metadata
includes `thinkingLevelMap`, `headers` and `compat`. Preserve unfamiliar fields;
do not infer a model's capabilities from its ID. See the
[model records](models/api.md) and [catalog](catalog.md).

The cycle examples have one available non-reasoning model. With multiple models,
`cycle_model` returns `{"model":MODEL,"thinkingLevel":"off","isScoped":false}`;
`MODEL` means the complete object shown above, not a wire string. Scoped cycling
sets `isScoped` to true. Thinking levels are `off`, `minimal`, `low`, `medium`,
`high` and `xhigh`; the effective level depends on the selected model. A successful
setter does not guarantee the model supports the requested level. Read state
back. A reasoning model's thinking cycle returns an object such as
`{"level":"low"}` instead of null.

### Submit, interrupt and queue work

```json
{"id":"p","type":"prompt","message":"Check the fixture"}
{"id":"p","type":"response","command":"prompt","success":true}
{"id":"s","type":"steer","message":"Report only failures"}
{"id":"s","type":"response","command":"steer","success":true}
{"id":"f","type":"follow_up","message":"Then list the files checked"}
{"id":"f","type":"response","command":"follow_up","success":true}
{"id":"a","type":"abort"}
{"id":"a","type":"response","command":"abort","success":true}
```

These requests need not be sent together. While a turn runs, `steer` queues input
for the next model call after the current tool calls finish. `follow_up` waits
until the agent has no more tool calls or steering input. `abort` waits for the
agent to become idle; it does not erase queued input.

A `prompt` during streaming must choose `streamingBehavior`: `steer` or
`followUp`. Without it the request fails. Extension slash commands sent through
`prompt` can run immediately during streaming; `steer` and `follow_up` reject
extension commands. Skill commands and prompt templates expand before delivery.
See [application prompting](application/session-operations.md) for the session rules.

All three input commands accept optional `images`, an array of image blocks:
`{"type":"image","data":"BASE64_BYTES","mimeType":"image/png"}`. Replace
`BASE64_BYTES` with actual encoded image bytes; it is not a usable image.

```json
{"id":"steering-mode","type":"set_steering_mode","mode":"all"}
{"id":"steering-mode","type":"response","command":"set_steering_mode","success":true}
{"id":"follow-mode","type":"set_follow_up_mode","mode":"one-at-a-time"}
{"id":"follow-mode","type":"response","command":"set_follow_up_mode","success":true}
```

Both queues default to `one-at-a-time`: one steering item per delivery point,
one follow-up item per completed run. `all` delivers the whole corresponding
queue at that point. `queue_update` carries the current pending text lists, not
an incremental edit:

```json
{"type":"queue_update","steering":["Report only failures"],"followUp":["Then list the files checked"]}
```

Replace your queue display with those lists. Delivery removes the corresponding
text and emits another update.

### Run a shell command and inspect accounting

```json
{"id":"shell","type":"bash","command":"printf 'ready\\n'"}
{"id":"shell","type":"response","command":"bash","success":true,"data":{"output":"ready\n","exitCode":0,"cancelled":false,"truncated":false}}
{"id":"stop-shell","type":"abort_bash"}
{"id":"stop-shell","type":"response","command":"abort_bash","success":true}
{"id":"stats","type":"get_session_stats"}
{"id":"stats","type":"response","command":"get_session_stats","success":true,"data":{"sessionId":"session-a","userMessages":1,"assistantMessages":1,"toolCalls":0,"toolResults":0,"totalMessages":3,"tokens":{"input":20,"output":2,"cacheRead":0,"cacheWrite":0,"total":22},"cost":0,"contextUsage":{"tokens":22,"contextWindow":128000,"percent":0.017187499999999998}}}
```

The statistics example follows one shell command, one user prompt and one
assistant answer. `totalMessages` includes application messages such as shell
output. Saved sessions also include `sessionFile`. Token totals aggregate
assistant usage in the current conversation; `cost` is the sum of reported
costs. `contextUsage` estimates the active context rather than lifetime usage;
it is absent without a context window. Its `tokens` and `percent` can be null
after compaction until fresh assistant usage arrives.

A nonzero shell exit code is still a successful RPC command. Inspect `exitCode`,
`cancelled` and `truncated` yourself. `exitCode` may be absent or null when no
numeric status is available. Truncated output can include `fullOutputPath`, the
file containing complete output. That path belongs to the server's filesystem;
do not assume a remote client can open it.

The shell result becomes a `bashExecution` conversation message without a
`message_start`/`message_end` pair for that addition. The next model prompt
includes it as user-facing command/output text unless excluded from context.
It is separate from a model-invoked `bash` tool, which emits tool execution
events. See [shell execution](tools.md) for output and cancellation policy.

### Compact and control retries

```json
{"id":"auto-compact","type":"set_auto_compaction","enabled":false}
{"id":"auto-compact","type":"response","command":"set_auto_compaction","success":true}
{"id":"compact","type":"compact","customInstructions":"Keep the check result"}
{"type":"compaction_start","reason":"manual"}
{"type":"compaction_end","reason":"manual","result":{"summary":"Checked","firstKeptEntryId":"entry-a","tokensBefore":22,"details":{"readFiles":[],"modifiedFiles":[]}},"aborted":false,"willRetry":false}
{"id":"compact","type":"response","command":"compact","success":true,"data":{"summary":"Checked","firstKeptEntryId":"entry-a","tokensBefore":22,"details":{"readFiles":[],"modifiedFiles":[]}}}
{"id":"retry","type":"set_auto_retry","enabled":false}
{"id":"retry","type":"response","command":"set_auto_retry","success":true}
{"id":"stop-retry","type":"abort_retry"}
{"id":"stop-retry","type":"response","command":"abort_retry","success":true}
```

Compaction calls the selected model and can incur charges. Its summary and entry
ID depend on the conversation; `details` is optional JSON: built-in compaction
records the files it read and modified, and an extension's compaction supplies its
own data.
Automatic compaction also emits these events with `reason` `threshold` or
`overflow`. A cancelled compaction omits `result` and has `aborted:true`. A failed
one omits `result` and has `aborted:false` and an `errorMessage`. An overflow recovery
can set `willRetry:true`: the agent will run again after compaction.

When automatic retry is enabled, transient provider failures can produce:

```json
{"type":"auto_retry_start","attempt":1,"maxAttempts":3,"delayMs":2000,"errorMessage":"503 unavailable"}
{"type":"auto_retry_end","success":true,"attempt":1}
```

Exhaustion instead ends with, for example:

```json
{"type":"auto_retry_end","success":false,"attempt":3,"finalError":"503 unavailable"}
```

These numbers are examples, not fixed retry settings. `abort_retry` cancels the
retry delay. A failed assistant message can precede a retry, so do not close the
connection on the first error message. Likewise `agent_end` closes one run;
automatic retry or overflow recovery can start another. Keep reading the retry
and compaction events if your client enables those policies. The Python example
is intentionally limited to one run. See [retry and compaction](application/session-operations.md).

### Save, fork or replace the session

```json
{"id":"name","type":"set_session_name","name":"RPC check"}
{"id":"name","type":"response","command":"set_session_name","success":true}
{"id":"forkable","type":"get_fork_messages"}
{"id":"forkable","type":"response","command":"get_fork_messages","success":true,"data":{"messages":[{"entryId":"entry-a","text":"Check the fixture"}]}}
{"id":"fork","type":"fork","entryId":"entry-a"}
{"id":"fork","type":"response","command":"fork","success":true,"data":{"text":"Check the fixture","cancelled":false}}
{"id":"clone","type":"clone"}
{"id":"clone","type":"response","command":"clone","success":true,"data":{"cancelled":false}}
{"id":"new","type":"new_session","parentSession":"/work/sessions/parent.jsonl"}
{"id":"new","type":"response","command":"new_session","success":true,"data":{"cancelled":false}}
{"id":"switch","type":"switch_session","sessionPath":"/work/sessions/saved.jsonl"}
{"id":"switch","type":"response","command":"switch_session","success":true,"data":{"cancelled":false}}
```

Use real IDs and paths from your session. `fork` selects the point before the
chosen user message and returns text the client can put back in its input.
`clone` copies the current active branch at its current entry. It fails on an
empty session. An extension can cancel a replacement: inspect `cancelled` even
when `success` is true. A cancelled fork omits `text`. Read `get_state` again
after replacement to update your session identity and path.

To request an HTML document:

```json
{"id":"html","type":"export_html","outputPath":"/work/session.html"}
{"id":"html","type":"response","command":"export_html","success":true,"data":{"path":"/work/session.html"}}
```

The parent directory must exist. Export can overwrite an existing destination;
choose a disposable path when testing. Omitting `outputPath` selects the
exporter's default filename. This needs the prepared offline viewer bundle;
a checkout or installation without it fails with a file-read error. Viewer
delivery is tracked in [#1762](https://github.com/Orchestration-Maestro/maestro/issues/1762)
and distribution packaging in
[#2847](https://github.com/Orchestration-Maestro/maestro/issues/2847).
See [session documents](export.md) for required assets and output naming.

## Read conversation records

`get_messages`, message events, `turn_end` and `agent_end` share the same message
records. Timestamps are numeric milliseconds. Keep role distinctions: a tool
result is not an assistant answer, and a compaction summary is not a new user
prompt.

A user message has plain text or ordered text/image blocks:

```json
{"role":"user","content":[{"type":"text","text":"Check the fixture"}],"timestamp":1700000000000}
```

An assistant message includes the selected provider/API/model, ordered content,
usage and its stop reason:

```json
{"role":"assistant","content":[{"type":"text","text":"Checked"}],"api":"openai-completions","provider":"fixture","model":"controlled","responseId":"fixture-response","usage":{"input":20,"output":2,"cacheRead":0,"cacheWrite":0,"totalTokens":22,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1700000000001}
```

Assistant content can also contain `{"type":"thinking","thinking":"..."}` or
`{"type":"toolCall","id":"call-a","name":"read","arguments":{"path":"fixture.txt"}}`.
Text can carry `textSignature`; thinking can carry `thinkingSignature` and
`redacted`; tool calls can carry `thoughtSignature`. Preserve these optional
provider metadata fields when retaining messages. `arguments` is a JSON value,
not necessarily an object or a string to parse.

Optional assistant fields are `responseModel`, `responseId`, `diagnostics` and
`errorMessage`. Stop reasons are `stop`, `length`, `toolUse`, `error` and
`aborted`. An error is still an assistant message; display its `errorMessage`
when present. The usage member `totalTokens` can be missing or null; do not
replace a missing total with a computed one. The cost object's `total` and the
statistics token object's `total` are distinct from it.
See [assistant output](models/assistant-output.md) for diagnostics and metadata.

A model tool result has a call identity, text/image content and optional details:

```json
{"role":"toolResult","toolCallId":"call-a","toolName":"read","content":[{"type":"text","text":"fixture contents"}],"details":{},"isError":false,"timestamp":1700000000002}
```

Shell RPC commands add this separate application message:

```json
{"role":"bashExecution","command":"printf 'ready\\n'","output":"ready\n","exitCode":0,"cancelled":false,"truncated":false,"timestamp":1700000000003}
```

Its optional members are `exitCode`, `fullOutputPath` and `excludeFromContext`.
Absent and null are distinct for these fields. Other application roles are:

```json
{"role":"custom","customType":"check-status","content":"Fixture checked","display":true,"details":{"files":1},"timestamp":1700000000004}
{"role":"branchSummary","summary":"Checked the fixture","fromId":"entry-a","timestamp":1700000000005}
{"role":"compactionSummary","summary":"Checked the fixture","tokensBefore":22,"timestamp":1700000000006}
```

Custom content uses the same text-or-block format as user content; `details`
may be absent or null. The application owns these records; see
[session messages](sessions/messages.md).

## Render a streamed answer

Dispatch by the outer `type` first. On `message_start`, create the message view;
on `message_update`, update it; on `message_end`, retain the completed record.
Each update contains both a current `message` snapshot and an
`assistantMessageEvent`. Choose either snapshot replacement or delta application
for rendering; doing both appends the same text twice.

For an answer of `Checked`, the text delta inside a `message_update` is:

```json
{"type":"text_delta","contentIndex":0,"delta":"Checked","partial":{"role":"assistant","content":[{"type":"text","text":"Checked"}],"api":"openai-completions","provider":"fixture","model":"controlled","usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}},"stopReason":"stop","timestamp":1700000000001}}
```

The delta is nested at `assistantMessageEvent`, not a top-level RPC record.
`contentIndex` identifies a content block, not a character offset.

| Inner event | Fields beyond `type` |
|---|---|
| `text_start`, `thinking_start`, `toolcall_start` | `contentIndex`, `partial` |
| `text_delta`, `thinking_delta`, `toolcall_delta` | `contentIndex`, `delta`, `partial` |
| `text_end`, `thinking_end` | `contentIndex`, `content`, `partial` |
| `toolcall_end` | `contentIndex`, `toolCall`, `partial` |

Tool-call deltas can contain incomplete JSON. Wait for `toolcall_end.toolCall`
for complete arguments rather than trying to execute partial text.

For assistant completion, inspect `message_end.message.stopReason`: `stop` and
`length` end the answer, `toolUse` requests tool execution, and `error` or
`aborted` indicates failure or cancellation. Display `message.errorMessage`
when present. A `message_end` completes one message, not the whole run. The run
may execute tools and request another assistant message before `agent_end`.

Correlate `tool_execution_start`, `tool_execution_update` and
`tool_execution_end` by `toolCallId`. Start and update include `toolName` and
`args`; update adds `partialResult`, while end adds `result` and `isError`.
A result contains `content` (text/image blocks) and optional `details`.
Partial results represent the tool's current result, so replace the preview
rather than concatenating it. Do not confuse these tool events with the
assistant's tool-call argument deltas.

`turn_end` includes the completed assistant `message` and `toolResults` for that
turn. `agent_end.messages` contains the messages generated in the run. Keep
reading after command acceptance to receive them. A failed preflight instead
produces a failed `response` and may never produce `agent_end`.

Protocol failures and extension failures need separate handling:

```json
{"id":"missing","type":"set_model","provider":"absent","modelId":"unknown"}
{"id":"missing","type":"response","command":"set_model","success":false,"error":"Model not found: absent/unknown"}
{"type":"extension_error","extensionPath":"/work/extensions/check.wasm","event":"tool_call","error":"Check failed"}
```

The first failure belongs to a request; the second is an unsolicited extension
event. Neither closes the stream. Keep stderr available for process-level
failures that cannot be reported over a broken output pipe.

## Events

The server streams these session events as they happen, without an `id`:

| Event | Members |
|---|---|
| `agent_start` | |
| `agent_end` | `messages` |
| `turn_start` | |
| `turn_end` | `message`, `toolResults` |
| `message_start` | `message` |
| `message_update` | `assistantMessageEvent`, `message` |
| `message_end` | `message` |
| `tool_execution_start` | `toolCallId`, `toolName`, `args` |
| `tool_execution_update` | `toolCallId`, `toolName`, `args`, `partialResult` |
| `tool_execution_end` | `toolCallId`, `toolName`, `result`, `isError` |
| `queue_update` | `steering`, `followUp` |
| `compaction_start` | `reason` |
| `compaction_end` | `reason`, `aborted`, `willRetry`, optional `result` and `errorMessage` |
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
zero or absent timeout waits, and a negative timeout expires immediately. An
`editor` has no timeout. A dialog whose signal already aborted writes nothing
and returns its default. A dialog settles with whatever came first:
a response that arrives after the abort or the deadline is ignored.

Dialogs work while the first session is still binding: a `session_start` handler can ask
one, and its answer finishes the binding, even when it is the input's final line. When the
input ends or fails before the answer, no answer can arrive: the dialog settles as cancelled,
any dialog the binding asks after that settles as cancelled and writes no request, and the
binding finishes. Other commands that arrive meanwhile wait and then run in arrival
order.

For example, a selection and its answer are two independent records on opposite
pipes:

```json
{"type":"extension_ui_request","id":"7f1690fb-39f3-4875-8a9c-fbd9202c9776","method":"select","title":"Choose check scope","options":["Changed files","All files"],"timeout":10000}
{"type":"extension_ui_response","id":"7f1690fb-39f3-4875-8a9c-fbd9202c9776","value":"Changed files"}
```

For `input` or `editor`, use the same `value` member for the entered text,
including embedded newlines encoded as `\n`. For a confirmation:

```json
{"type":"extension_ui_request","id":"eaad1508-cce0-4cb2-8033-fb93eff26ede","method":"confirm","title":"Run check?","message":"Check all files?"}
{"type":"extension_ui_response","id":"eaad1508-cce0-4cb2-8033-fb93eff26ede","confirmed":true}
```

To dismiss any pending dialog:

```json
{"type":"extension_ui_response","id":"eaad1508-cce0-4cb2-8033-fb93eff26ede","cancelled":true}
```

Dismissal, timeout and abort return no value for selection, input or editor,
and false for confirmation. Dialog responses do not get a command response.
Dispatch them separately from the ordinary request-ID map. Handle dialogs even
before your first command's response, because session startup can wait on one.
The Python example cancels every answered dialog rather than blocking startup.

An extension can also send notices without waiting:

```json
{"type":"extension_ui_request","id":"notice-a","method":"notify","message":"Check complete","notifyType":"info"}
{"type":"extension_ui_request","id":"notice-b","method":"setStatus","statusKey":"check","statusText":"Ready"}
{"type":"extension_ui_request","id":"notice-c","method":"setWidget","widgetKey":"check","widgetLines":["Fixture checked"],"widgetPlacement":"aboveEditor"}
{"type":"extension_ui_request","id":"notice-d","method":"setTitle","title":"Maestro check"}
{"type":"extension_ui_request","id":"notice-e","method":"set_editor_text","text":"Check another fixture"}
```

Actual notice IDs are fresh UUIDs too. `notifyType` supports `info`, `warning`
and `error`; `widgetPlacement` supports `aboveEditor` and `belowEditor`.
Omitted `statusText` or `widgetLines` clears the corresponding entry. These
notices do not change your application's display unless your client renders
them. Never send an answer to a notice.

Extensions have UI access through this protocol, but no terminal component tree.
Widget factories, custom components, footers, headers, editor components and
autocomplete providers are never built here. Working-message/indicator setters,
hidden-thinking labels and tool-expansion setters have no visible effect.
Editor text reads as an empty string, tool expansion as false, theme listings as
an empty list and named theme lookup as no result. Pasting forwards to
`set_editor_text`; it has no terminal paste/collapse behavior. The live theme
remains available to extensions, but theme selection answers
`Theme switching not supported in RPC mode`.

## Ending

The connection ends when the input ends, when an extension requests shutdown, or on SIGTERM
or SIGHUP. At the end of input the server stops streaming events, starts disposing the
session and answers a final line without its LF; the process exits 0. A shutdown an
extension requests takes effect after the next command's response. The first signal kills
the shell commands still running, disposes the session and exits with 143 or 129. Once
shutdown has started, a second signal kills the process by that signal; once the session
is disposing, the end of input exits 0 at once. A failed read, or a failed write of any
record, also disposes the session, and the connection returns that first failure however
it then ends.

## Startup handoff

The application-owned `presentation_data::mode_dispatch::RpcModeRunner` accepts
the prepared session runtime. A native adapter supplies `RpcModeOptions` and
calls `run_rpc_mode`; the runner retains its standard-stream framing and exit
policy. The `maestro` executable wires that adapter to command-line startup.
