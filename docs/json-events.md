# JSON session events

Use `--mode json` to run prompts to completion while another program reads the
session's events. Standard output is newline-delimited JSON, not a single JSON
document and not a terminal transcript. For a long-lived child that accepts
commands, use [RPC](rpc.md) instead: JSON output does not accept RPC requests.
The [JSON library page](json.md) describes serialization helpers, not this mode.

## Contents

1. [Capture a run](#capture-a-run)
2. [Read the records](#read-the-records)
3. [Follow streaming content](#follow-streaming-content)
4. [Handle every message role](#handle-every-message-role)
5. [Decide whether the run succeeded](#decide-whether-the-run-succeeded)

## Capture a run

1. Set up the executable and credentials using the [first-session guide](first-session.md).
   The example selects `openai/gpt-4o-mini`; choose a configured model you can use.
   A real provider can charge for this request. The guide was checked with a
   disposable configuration and a local HTTP provider, without live credentials.
2. Open a disposable working directory. The following command replaces
   `events.jsonl` and `diagnostics.log` there. Run it from a shell with `maestro`
   available:

   ```sh
   maestro --mode json --model openai/gpt-4o-mini \
     --no-session --no-tools --no-extensions --no-skills \
     --no-prompt-templates --no-context-files \
     "Reply with a short greeting." > events.jsonl 2> diagnostics.log
   status=$?
   printf 'Maestro exit: %s\n' "$status"
   ```

   `--mode json` selects the non-interactive runner without an extra `--print`.
   `--no-session` avoids saving the conversation; it does not remove the header
   from this output. The other switches disable tools and discovery of extensions,
   skills, prompt templates and context files for this example. They are not a
   filesystem or network sandbox. See [command-line input](cli.md) for input
   preparation and [print mode](print.md#options) for multiple prompts.
3. With [jaq](https://github.com/01mf02/jaq) installed, select completed assistant
   messages without mixing in the user's prompt or tool results:

   ```sh
   jaq -c 'select(.type == "message_end" and .message.role == "assistant")' events.jsonl
   ```

   Keep the full capture until you have checked its outcome. A filtered stream
   alone loses retry, tool and lifecycle information. Captures can contain source,
   images, thinking and tool output; review them before sharing.

## Read the records

Read one complete line at a time and decode each line separately. Do not split
on braces or expect every read from a pipe to contain a whole record: escaped
newlines belong inside JSON strings, and a pipe read can stop mid-record.

The initial record is the current session header, with `type: "session"`,
`version`, `id`, `timestamp` and `cwd`; a fork can also carry `parentSession`.
New sessions use format version 3. The timestamp is text and the directory is
the session's working directory. See the [header format](sessions/session_format.md#header)
and its [record owner](../crates/maestro-entry/src/session_manager.rs).

For example, the controlled run began with these three complete records:

```json
{"type":"session","version":3,"id":"01a17f3e-324a-757e-b029-163430a57b78","timestamp":"2026-10-27T21:22:24.970Z","cwd":"/tmp/maestro-json-guide-wwmpqwr9"}
{"type":"agent_start"}
{"type":"turn_start"}
```

The ID, timestamp and working directory will differ in your run.
Subsequent lines are live events. This is not the saved session file's entry
tree: do not expect an entry `id` or `parentId` on each event. Continuing a
session does not replay its saved entries as live events. The header is written
before extension binding; events emitted during the initial binding are not yet
subscribed. See [print output](print.md#output) for that boundary.

For an ordinary text reply, expect agent and turn starts, the user message's
start/end, the assistant message's start/updates/end, then turn and agent ends.
Tool use can add more turns. Do not assume that every run contains every event
below or a fixed number of updates.

The session event owner is
[`AgentSessionEvent`](../crates/maestro-app/src/agent_session/types.rs).
Wire member names are camelCase; event `type` values are snake_case.

| Session event | Members and meaning |
| --- | --- |
| `queue_update` | `steering` and `followUp` are the complete pending text queues, replacing the previous queue view. |
| `compaction_start` | `reason` is `manual`, `threshold` or `overflow`. |
| `compaction_end` | `reason`, optional `result`, `aborted`, `willRetry` and optional `errorMessage` describe the outcome. |
| `auto_retry_start` | `attempt`, `maxAttempts`, `delayMs` and `errorMessage` describe a scheduled retry. |
| `auto_retry_end` | `success`, `attempt` and optional `finalError` close the retry sequence. |
| `session_info_changed` | Optional `name` reports the session name now in effect. |
| `thinking_level_changed` | `level` reports the effective thinking level. |

The same stream includes all base
[`AgentEvent`](../crates/maestro-agent/src/types/events.rs) variants, without an
extra wrapper around them:

| Base event | Members and meaning |
| --- | --- |
| `agent_start` | Begins the conversation operation; no payload. |
| `agent_end` | `messages` contains newly admitted messages, not the whole saved history. |
| `turn_start` | Begins an assistant turn; no payload. |
| `turn_end` | `message` is the assistant response; `toolResults` contains that turn's tool results in call order. |
| `message_start` | `message` begins a user, assistant or tool-result entry. |
| `message_update` | `message` is an updated assistant snapshot; `assistantMessageEvent` describes the content update. |
| `message_end` | `message` is the completed entry. Inspect its role before consuming it as an answer. |
| `tool_execution_start` | `toolCallId`, `toolName`, `args` identify the requested call and arguments. |
| `tool_execution_update` | The same call fields plus `partialResult` report progress. |
| `tool_execution_end` | `toolCallId`, `toolName`, `result`, `isError` close the call. This event does not include `args`. |

A tool-execution result has `content` and `details`; the conversation's
`toolResult` message also records its call ID, tool name, error flag and time.
Correlate tool observations with `toolCallId`, not the tool name: a turn may
invoke the same tool more than once.

## Follow streaming content

A `message_update` carries both the current assistant `message` and the nested
`assistantMessageEvent`. Choose one display strategy: replace your rendered
message from snapshots, or append deltas by `contentIndex`. Appending both
repeats text. A snapshot can already contain more content than an individual
delta; use the completed `message_end` as the final message.

To extract only text fragments from the captured run, without adding a newline
after each fragment:

```sh
jaq -j 'select(.type == "message_update" and .assistantMessageEvent.type == "text_delta") | .assistantMessageEvent.delta' events.jsonl
```

This filter also works as a live consumer of stdout. A fragment need not be a
word, line or complete JSON argument. Keep tool-call fragments as text until
the call is complete; do not execute partial arguments.

Nested updates use these fields:

| Nested type | Members |
| --- | --- |
| `text_start`, `thinking_start`, `toolcall_start` | `contentIndex`, `partial` |
| `text_delta`, `thinking_delta`, `toolcall_delta` | `contentIndex`, `delta`, `partial` |
| `text_end`, `thinking_end` | `contentIndex`, `content`, `partial` |
| `toolcall_end` | `contentIndex`, `toolCall`, `partial` |

Here `toolcall_delta` is spelled differently from the message content block's
`toolCall`. The [model event owner](../crates/maestro-request/src/types/events.rs)
also defines `start`, `done` and `error` for provider streams. The agent consumes
those lifecycle events to produce `message_start` and `message_end`; do not wait
for them inside a JSON-mode `message_update`.

## Handle every message role

Dispatch on `message.role`, separately from the outer event's `type`. Standard
message records belong to
[`maestro-request::types`](../crates/maestro-request/src/types/messages.rs),
re-exported through models. Application message records belong to
[`maestro-request::messages`](../crates/maestro-request/src/messages.rs),
re-exported through session; see [application records](records.md#application-message-records)
and [conversation projection](sessions/messages.md).

| Role | Content to retain |
| --- | --- |
| `user` | `content` is text or an ordered array of text/image blocks; `timestamp` is numeric. |
| `assistant` | `content` contains `text`, `thinking` or `toolCall` blocks, with provider/API/model identity, `usage`, `stopReason`, numeric `timestamp` and optional response metadata, diagnostics or `errorMessage`. |
| `toolResult` | `toolCallId`, `toolName`, text/image `content`, optional `details`, `isError`, numeric `timestamp`. |
| `bashExecution` | `command`, `output`, `cancelled`, `truncated`, numeric `timestamp`; optional `exitCode`, `fullOutputPath`, `excludeFromContext`. |
| `custom` | `customType`, text or text/image `content`, `display`, numeric `timestamp`, optional `details`. |
| `branchSummary` | `summary`, `fromId`, numeric `timestamp`. |
| `compactionSummary` | `summary`, `tokensBefore`, numeric `timestamp`. |

An image block carries base64 `data` and `mimeType`; thinking text uses
`thinking`, not `text`. A tool-call block carries `id`, `name` and `arguments`.
Do not render all content as strings or treat application messages as assistant
answers. Preserve optional fields as supplied: some application fields distinguish
missing from explicit null. An event stream reports what occurs during the run;
this role inventory does not mean each role will appear in a greeting request.

## Decide whether the run succeeded

Keep stdout and stderr separate. Do not use `2>&1` when feeding a JSON decoder:
startup and extension diagnostics are ordinary text on stderr. Extension errors
can be reported there without making the run fail. A startup failure can leave
stdout empty; an encoding, write or prompt-operation failure can leave only a
prefix of the event stream.

Use both the process status and completed assistant messages:

| Observation | Meaning |
| --- | --- |
| Exit 0 | The JSON runner completed. This alone does not prove the assistant succeeded. |
| Assistant `stopReason: "stop"`, `"length"` or `"toolUse"` | The response stopped normally, reached its limit or requested tools. A tool-use response is not necessarily the final answer. |
| Assistant `stopReason: "error"` or `"aborted"` | A represented response failure. Inspect `errorMessage` when present. JSON mode can still exit 0; retry events can precede a later successful response. |
| Exit 1 | Startup or a run operation failed, including event encoding/writing. Read stderr and treat an incomplete capture as incomplete. |
| Exit 128 plus signal number | Print-mode termination cleanup has settled after a termination signal. |

For example, the controlled provider check returned HTTP 401: the capture
contained an assistant with `stopReason: "error"` while the process returned 0.
A missing input file instead returned 1, wrote a diagnostic to stderr and
produced no JSON records. Do not look only for an `error` outer event: that is
not a session event type.

Wait for the process to exit before accepting the capture. `agent_end` closes a
conversation operation, not necessarily the process: another supplied prompt or
cleanup can follow. A failed disposal or final stdout flush also fails the
process. [Print cleanup](print.md#errors-and-cleanup) owns these rules. Text print
mode has a different policy for represented assistant failures; use the JSON
policy above when building an event consumer.
