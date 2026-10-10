# Writing an extension as a component

`maestro-extensions-wasm` is the Rust library an extension author links to. It owns the
canonical interface files in `crates/maestro-extensions-wasm/wit/` (package
`maestro:extension`) and a facade over the generated bindings, so an extension is ordinary
async Rust that the host runs as a WebAssembly component. The facade uses `maestro-request` for shared model and resource records.
This page describes what is delivered so far: registration, 29 events and thirteen result
families, tool callbacks, command contexts, session continuations and session readers.

## Build a component

```sh
mise exec -- just extension-author-component
```

The recipe adds the `wasm32-wasip2` target and builds the example in
`crates/maestro-extensions-wasm/examples/author_component.rs` as a component. An extension
crate is a `cdylib` whose root names its factory:

```rust
maestro_extensions_wasm::export_extension!(factory);

fn factory(api: maestro_extensions_wasm::ExtensionAPI)
    -> maestro_extensions_wasm::ExtensionFuture<'static, ()>
{
    Box::pin(async move { api.on("input", handler) })
}
```

The macro defines the exported entry point; the crate needs no raw bindings. A native build
of the example links an empty library, so only the `wasm32` build produces a component.

## Register

Each registration reaches the host through its imports at the moment the factory makes it,
so the host has seen it before the
factory's next statement runs. Whether and when the host treats the extension as active is
the host's decision; the `start` export resolves when the factory's future completes. `on`
forwards each handler under its event name, including names the host does not define, and
`register_command` forwards a command and `register_tool` forwards one tool definition. A rejected registration returns the host's message and
retains nothing.

Each closure you register stays alive in the guest until the host releases it, exactly once,
and never while the guest's closure table is borrowed. A destructor may register again while
its own closure is released.

## Tools

`ToolDefinition` combines `ToolMetadata`, optional synchronous `prepare_arguments` and
asynchronous `execute`. Preparation receives one JSON value; its returned value can be
passed to execution. The adapter does not perform argument validation or schedule tools.
Execution receives the invocation ID, arguments, ordinary context and optional signal
and progress callback. A progress callback sends one `AgentToolResult` synchronously;
execution returns its final `AgentToolResult`. Callback failures retain their supplied messages.

Cloned signal and progress handles share the resource lent for that invocation. Returning
from execution does not revoke retained handles; their last owner releases them.
For shared content and JSON conventions, see [the data boundary](#events-and-results).

`ToolCallEvent` input and `ToolResultEvent` details retain their JSON independently of
`toolName`, including overrides of built-in names. Their `ToolInput` / `ToolDetails`
carrier offers `decode::<T>()` for an on-demand typed view and `from_value()` for
finite-checked authored replacements. Named aliases expose the built-in typed views;
name predicates compare the name alone. Result input remains an opaque string.
`UserBashEventResult` carries an already-produced `BashResult`; this crate does not
execute shell commands.

## Session readers

Ordinary and command contexts expose `session_manager()`, which acquires the currently
bound `ReadonlySessionManager` on each call. Its thirteen queries forward the host's
strings, optional records and ordered lists without guest-side trimming, sorting or
path resolution. Entry records use [the existing data boundary](#events-and-results).

`get_tree()` returns independently owned `SessionTreeNode` handles. Read a node's
`entry()`, `children()`, `label()` or `label_timestamp()` to traverse the captured tree;
the guest does not decode a complete recursive tree document. Extracted readers and
nodes survive dropping or invalidating the context that supplied them. Reader clones
share one owner, released when its last alias is dropped; descendants can outlive parents.

Queries return `ExtensionResult`: host failures retain their messages, while selected
record decoding failures return native JSON diagnostics. Catching a failure does not
revoke the reader. The read-only facade exposes no session-writing operations.

## Events and results

Rust records define the event payloads and results; the interface files define how a callback
is invoked and which host resources it is lent. A handler receives `&mut ExtensionEvent` and
an `ExtensionContext` and answers with `Option<ExtensionEventResult>` or a message. No result
differs from a result whose properties are all omitted.

| `type` | Rust event | Properties (`?` marks an optional one) |
| --- | --- | --- |
| `context` | `ContextEvent` | `messages` |
| `before_agent_start` | `BeforeAgentStartEvent` | `prompt`, `images?`, `systemPrompt`, `systemPromptOptions` |
| `agent_start` | `AgentStartEvent` | none |
| `agent_end` | `AgentEndEvent` | `messages` |
| `turn_start` | `TurnStartEvent` | `turnIndex`, `timestamp` |
| `turn_end` | `TurnEndEvent` | `turnIndex`, `message`, `toolResults` |
| `message_start` | `MessageStartEvent` | `message` |
| `message_end` | `MessageEndEvent` | `message` |
| `message_update` | `MessageUpdateEvent` | `message`, `assistantMessageEvent` |
| `tool_call` | `ToolCallEvent` | `toolCallId`, `toolName`, retained JSON `input` |
| `tool_result` | `ToolResultEvent` | `toolCallId`, `toolName`, opaque `input`, shared `content`, `isError`, retained JSON `details?` |
| `user_bash` | `UserBashEvent` | `command`, `cwd`, `excludeFromContext` |
| `tool_execution_start` | `ToolExecutionStartEvent` | `toolCallId`, `toolName`, `args` |
| `tool_execution_update` | `ToolExecutionUpdateEvent` | `toolCallId`, `toolName`, `args`, `partialResult` |
| `tool_execution_end` | `ToolExecutionEndEvent` | `toolCallId`, `toolName`, `result`, `isError` |
| `model_select` | `ModelSelectEvent` | `model`, `previousModel?`, `source` (`set`, `cycle`, `restore`) |
| `thinking_level_select` | `ThinkingLevelSelectEvent` | `level`, `previousLevel` (`off`, `minimal`, `low`, `medium`, `high`, `xhigh`) |
| `resources_discover` | `ResourcesDiscoverEvent` | `cwd`, `reason` (`startup`, `reload`) |
| `session_start` | `SessionStartEvent` | `reason` (`startup`, `reload`, `new`, `resume`, `fork`), `previousSessionFile?` |
| `session_before_switch` | `SessionBeforeSwitchEvent` | `reason` (`new`, `resume`), `targetSessionFile?` |
| `session_before_fork` | `SessionBeforeForkEvent` | `entryId`, `position` (`before`, `at`) |
| `session_before_compact` | `SessionBeforeCompactEvent` | `preparation`, `branchEntries`, `customInstructions?` |
| `session_compact` | `SessionCompactEvent` | `compactionEntry`, `fromExtension` |
| `session_before_tree` | `SessionBeforeTreeEvent` | `preparation` |
| `session_tree` | `SessionTreeEvent` | `newLeafId`, `oldLeafId`, `summaryEntry?`, `fromExtension?` |
| `session_shutdown` | `SessionShutdownEvent` | `reason` (`quit`, `reload`, `new`, `resume`, `fork`), `targetSessionFile?` |
| `before_provider_request` | `BeforeProviderRequestEvent` | `payload` |
| `after_provider_response` | `AfterProviderResponseEvent` | `status`, `headers` |
| `input` | `InputEvent` | `text`, `images?`, `source` (`interactive`, `rpc`, `extension`) |

Before-compaction `preparation` holds `firstKeptEntryId`, `messagesToSummarize`,
`turnPrefixMessages`, `isSplitTurn`, `tokensBefore`, `previousSummary?`, `fileOps` and
`settings`. `settings` carries `enabled`, `reserveTokens` and `keepRecentTokens`;
`fileOps` carries insertion-ordered `read`, `written` and `edited` path sets.
The guest does not normalize authored paths. `branchEntries` is independent of the
prepared message lists.

Before-tree `preparation` holds `targetId`, nullable `oldLeafId` and
`commonAncestorId`, `entriesToSummarize`, `userWantsSummary`, `customInstructions?`,
`replaceInstructions?` and `label?`. After-tree leaf IDs are also nullable; an absent
summary is not a fabricated summary entry. The eight session events are variants of
`SessionEvent`. `images` is a list of `data` and
`mimeType` records tagged with `type: "image"`, and `headers` a list of name and value pairs in the order the host gave
them; nothing trims, folds, sorts or combines them.

The agent payloads use shared user, assistant and tool-result messages plus application
`bashExecution`, `custom`, `branchSummary` and `compactionSummary` messages. Unknown roles
use `CustomAgentMessages` with a role and opaque `data` string. A decoding failure for a
known role does not fall back to custom data.

`message_update` carries all twelve shared model-stream variants. Its outer agent message
is independent of the stream message. The stream handle is detached after the callback,
before output validation and encoding; retained handles cannot change that output snapshot.
Guest snapshots do not provide live identity propagation between component values.
Model selection uses shared descriptors and their host decoding; see
[`ModelCompat`](https://docs.rs/maestro-request/latest/maestro_request/types/enum.ModelCompat.html)
for compatibility decoding.
Reasoning selection carries the current and previous levels without checking model support.

`BuildSystemPromptOptions` carries `cwd` and optional `customPrompt`, `selectedTools`,
`toolSnippets`, `promptGuidelines`, `appendSystemPrompt`, `contextFiles` and `skills`.
`toolSnippets` is a string-keyed ordered map. These are already-assembled inputs;
the guest does not discover resources or build the prompt.

| Event | Result | Properties of the result |
| --- | --- | --- |
| `tool_call` | `ToolCallEventResult` | `block?`, `reason?` |
| `tool_result` | `ToolResultEventResult` | `content?`, opaque `details?`, `isError?` |
| `user_bash` | `UserBashEventResult` | supplied `result?` |
| `context` | `ContextEventResult` | `messages?` |
| `message_end` | `MessageEndEventResult` | `message?` |
| `before_agent_start` | `BeforeAgentStartEventResult` | `message?`, `systemPrompt?` |
| `resources_discover` | `ResourcesDiscoverResult` | `skillPaths?`, `promptPaths?`, `themePaths?` |
| `session_before_switch` | `SessionBeforeSwitchResult` | `cancel?` |
| `session_before_fork` | `SessionBeforeForkResult` | `cancel?`, `skipConversationRestore?` |
| `session_before_compact` | `SessionBeforeCompactResult` | `cancel?`, `compaction?` |
| `session_before_tree` | `SessionBeforeTreeResult` | `cancel?`, `summary?`, `customInstructions?`, `replaceInstructions?`, `label?` |
| `before_provider_request` | `BeforeProviderRequestEventResult` | a string holding the replacement request |
| `input` | `InputEventResult` | `action` is `continue`, `handled` or `transform`; a transform has `text` and `images?` |

Returned `compaction` holds `summary`, `firstKeptEntryId`, `tokensBefore` and `details?`.
A returned tree `summary` holds `summary` and optional opaque `details`. Cancellation
and summary overrides may be supplied together; the guest carries the decision without
executing navigation policy. Session start, session shutdown and provider
responses have no result type delivered here. The adapter does not check that a result
belongs to the family of the event it answers: the host reads it. A message-end replacement
may carry a different role; host reduction policy is not applied in the guest.
The before-agent result's custom message has `customType`, `content`, `display` and
optional opaque `details`, without a role or timestamp.

An `AbortSignal` is a capability, not a snapshot: keep it past the callback and read
`aborted()` later. Keeping the `ExtensionContext` works the same way. Both continue to read
the host resources lent for the invocation that delivered them, whatever later invocations
receive. What a handler does not keep is dropped when the invocation ends, including when
the delivery is refused.

## How an event travels

The guest exports one asynchronous function, `invoke-event`, for every event. The host passes
the callback, the event as one host-encoded JSON document whose `type` names the kind, and the resources
the event needs: a context, and for before-compaction or before-tree its owned signal.
A signal that comes with any other event is dropped. Resources never appear in the document.

Unshared optional properties keep three states apart: omitted, explicitly `null`, and present, including
empty strings, empty lists and `false`. In unshared records such a field is a
`Presence<T>`: it defaults to `Missing` when the property is absent and is skipped on
serialization only while it is `Missing`. Serializing `Missing` on its own, outside a property,
is an error; `Null` serializes as `null` and `Present` as its value.

Shared records retain their own optional-field serialization: ordinary `Option` fields
collapse missing and null, while existing nested options retain explicit null. They
have no added `Presence` wrapper.

Numbers use plain JSON with exact finite floating-point parsing, including signed zero.
Ordinary host nonfinite serialization writes null, which a required numeric decoder
may reject. `ToolInput::from_value` rejects Infinity or NaN before returning a carrier.
Other extension-assigned Infinity or NaN fails encoding with
`extension wrote a non-finite number (Infinity or NaN)`; the independent callback
failure or other encoded part is retained. Numeric-looking text remains text.

The request body of `before_provider_request`, its replacement, tool-execution notification arguments and
partial/final notification results, and application-message or compaction `details` are opaque text
carried as a string. The adapter never parses or reformats it, so
duplicate keys, spacing, exponents and any nesting depth reach the handler as authored. A
replacement `null` is text and differs from returning no result; `details` text `null` differs
from `details` null.

The event's `type` tag must be a JSON string, not a single-property object. The reason fields
of resource discovery, session start, session switch and session shutdown, the fork position
and the input source accept only their declared JSON string literals in event records, not
object variants. They still serialize as lowercase strings.

The export decodes the host-encoded document before it enters the handler. The outer event, before-compaction preparation and input-image list elements are read
from JSON objects. Tree preparation, settings and file-set records also accept Serde
positional arrays. Flattened session entries remain maps; the entry union reads the tag first,
then decodes that concrete record from the original JSON, including nested messages.
Concrete-entry tags are serialization metadata, not a second ingress validator.
Known duplicate members follow the owning decoder; flattened unknown members are
buffered and have no arbitrary-depth skipping guarantee. The outer tag reader
skips other properties, and each payload uses its owning record decoder. Shared records decode
through the same Serde derives used by the host, including their native record and union
admission; there is no additional validation of foreign input shapes or tolerance of
unrepresentable unread members. A decoding failure or a before-compaction/before-tree event without its signal makes
the export fail with an error message before handler entry. A callback identity that is unknown,
or registered for a command, does not fail the export: the outcome reports the lookup message
as the decision.

The outcome has two independent parts. `event` is the event as the handler left it, whether
it returned or failed, and is absent only when the handler replaced it with an event of another
kind, which includes another session event. `decision` is `returned` with the encoded result,
or `failed` with a message: the handler's own, or the lookup message. A failure to encode the event or the result is reported
in its own part, as a message, and never replaces the other part or the handler's failure.

### Session entries

Every entry carries `id`, nullable `parentId` and a string `timestamp`; the guest
interprets neither IDs nor timestamps. Typed null parent/leaf/ancestor IDs serialize
as present null, unlike omitted `Presence` fields. Their `Option` decoder also accepts
absence. The tagged `SessionEntry` variants carry these additional fields:

| `type` | Fields |
| --- | --- |
| `message` | `message` (shared or application message) |
| `thinking_level_change` | `thinkingLevel` (open text) |
| `model_change` | `provider`, `modelId` |
| `compaction` | `summary`, `firstKeptEntryId`, `tokensBefore`, `details?`, `fromHook?` |
| `branch_summary` | `fromId`, `summary`, `details?`, `fromHook?` |
| `custom` | `customType`, `data?` (opaque text) |
| `custom_message` | `customType`, `content`, `details?`, `display` |
| `label` | `targetId`, `label?` |
| `session_info` | `name?` |

The persisted compact event includes a full `CompactionEntry`, not just a returned
`CompactionResult`. Its `fromExtension` flag is independent of the entry's `fromHook`.
Entry/message lists preserve order and repeated elements; file sets preserve distinct
literal members in insertion order.

The guest does not compact, persist entries or navigate a tree. It returns edited
preparation and branch entries through the event outcome; later host execution must
apply those edits before continuing.

## Contexts and sessions

Handlers run with an `ExtensionContext`, which reads the working directory. Commands run with
an `ExtensionCommandContext`, which adds `wait_for_idle` and `new_session`; the ordinary
context has neither method, which the rustdoc examples check at compile time.

`new_session` takes a `with_session` continuation. The host runs it against a
`ReplacedSessionContext` bound to the replacement session while the operation is pending. The
host calls the continuation closure once. Captures the closure keeps drop when that call
returns; captures moved into the returned future drop when the future completes. The identity
the host announced for it lasts until the operation ends, and a continuation the host never
ran is released then. The facade forwards the host's answers: after the operation, `cwd` on the
context that started it returns whatever the host reports, which the test host makes a
stale-context message, while the replacement context keeps working. Errors are the plain text
the host supplies, so an author can catch one and continue.

## Example

`crates/maestro-extensions-wasm/src/tests/author.rs` registers an input handler that
upper-cases text, a before-compact handler that cancels an aborted compaction and reports the
state of the signal it kept from the previous one, a command that starts a new session and
waits for idle from its continuation, a handler the host rejects, and a handler whose
destructor registers another. Two more handlers edit their event: one trims an input in place
and fails when nothing is left, the other notes the tokens in a compaction preparation and
fails when the compaction was aborted. A probe handler does what the delivery tells it, in the
JSON document that the delivery gives as the working directory of its context: edit the event,
replace it, keep the context and signal, report what it kept, wait on a command context that
the `capture` command stored, and return a result or fail.

`src/tests.rs` runs the extension through the built component and through an in-process host
that runs the component adapter's own functions, and compares the two transcripts. The host
holds the continuation's wait for idle until the test saw it pending, so the command finishes
only after that wait resumes. Afterwards the test checks that every callback, including the
one a destructor registered, and every context and signal the host lent was dropped. The
tests in `src/tests/event_values.rs` and `src/tests/event_transport.rs` deliver their
event-property cases and every way an invocation can end, for the 23 event kinds they list, through
both hosts with the probe handler. `src/tests/session_events.rs` and
`src/tests/session_records.rs` cover the session event kinds and their records.


### A user-request summary handler

This executable handler summarizes only supplied user messages; block content becomes
`[complex]`, and text keeps its first 100 Unicode scalar values. It does not read a
session or choose compaction settings. Register it as `session_before_compact`:

```rust
use maestro_extensions_wasm::{
    AgentMessage, CompactionResult, ExtensionAPI, ExtensionEvent, ExtensionEventResult,
    ExtensionFuture, Message, Presence, SessionBeforeCompactResult, SessionEvent, UserContent,
};
use std::rc::Rc;

fn factory(api: ExtensionAPI) -> ExtensionFuture<'static, ()> {
    Box::pin(async move {
        api.on("session_before_compact", Rc::new(|event, _ctx| {
            Box::pin(async move {
                let ExtensionEvent::Session(SessionEvent::BeforeCompact(event)) = event else {
                    return Ok(None);
                };
                let requests = event.preparation.messages_to_summarize.iter()
                    .filter_map(|message| {
                        let AgentMessage::Message(message) = message else { return None; };
                        let Message::User(user) = message.as_ref() else { return None; };
                        let text = match &user.content {
                            UserContent::Text(text) => text.chars().take(100).collect::<String>(),
                            UserContent::Blocks(_) => "[complex]".to_owned(),
                        };
                        Some(format!("- {text}"))
                    }).collect::<Vec<_>>().join("\n");
                Ok(Some(ExtensionEventResult::SessionBeforeCompact(SessionBeforeCompactResult {
                    cancel: Presence::Missing,
                    compaction: Presence::Present(CompactionResult {
                        summary: format!("User requests:\n{requests}"),
                        first_kept_entry_id: event.preparation.first_kept_entry_id.clone(),
                        tokens_before: event.preparation.tokens_before,
                        details: Presence::Missing,
                    }),
                })))
            })
        }))
    })
}
```

For cancellation instead, return a `SessionBeforeCompactResult` with
`cancel: Presence::Present(true)` and `compaction: Presence::Missing` from a separate
handler. Do not put a summary return after an unconditional cancellation return.
The tests invoke the summary handler on empty, mixed-role and repeated inputs,
ASCII boundaries, emoji and combining marks through both adapters.
