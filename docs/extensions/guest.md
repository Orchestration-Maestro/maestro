# Writing an extension as a component

`maestro-extensions-wasm` is the Rust library an extension author links to. It owns the
canonical interface files in `crates/maestro-extensions-wasm/wit/` (package
`maestro:extension`) and a facade over the generated bindings, so an extension is ordinary
async Rust that the host runs as a WebAssembly component. The facade uses `maestro-request` for shared model and resource records.
This page describes what is delivered so far: registration, seventeen events and the
results of six of them, command contexts and session continuations.

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
`register_command` forwards a command. A rejected registration returns the host's message and
retains nothing.

Each closure you register stays alive in the guest until the host releases it, exactly once,
and never while the guest's closure table is borrowed. A destructor may register again while
its own closure is released.

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
| `resources_discover` | `ResourcesDiscoverEvent` | `cwd`, `reason` (`startup`, `reload`) |
| `session_start` | `SessionStartEvent` | `reason` (`startup`, `reload`, `new`, `resume`, `fork`), `previousSessionFile?` |
| `session_before_switch` | `SessionBeforeSwitchEvent` | `reason` (`new`, `resume`), `targetSessionFile?` |
| `session_before_fork` | `SessionBeforeForkEvent` | `entryId`, `position` (`before`, `at`) |
| `session_before_compact` | `SessionBeforeCompactEvent` | `preparation`, `customInstructions?` |
| `session_shutdown` | `SessionShutdownEvent` | `reason` (`quit`, `reload`, `new`, `resume`, `fork`), `targetSessionFile?` |
| `before_provider_request` | `BeforeProviderRequestEvent` | `payload` |
| `after_provider_response` | `AfterProviderResponseEvent` | `status`, `headers` |
| `input` | `InputEvent` | `text`, `images?`, `source` (`interactive`, `rpc`, `extension`) |

`preparation` holds `firstKeptEntryId`, `isSplitTurn`, `tokensBefore` and `previousSummary?`.
The five session events are variants of `SessionEvent`. `images` is a list of `data` and
`mimeType` records tagged with `type: "image"`, and `headers` a list of name and value pairs in the order the host gave
them; nothing trims, folds, sorts or combines them.

The agent payloads use shared user, assistant and tool-result messages. Unknown roles
use `CustomAgentMessages` with a role and opaque `data` string. Known model roles decode
through the shared records' Serde derives; a decoding failure does not fall back to custom
data. Application-defined bash, custom and summary roles are not delivered yet.

`BuildSystemPromptOptions` carries `cwd` and optional `customPrompt`, `selectedTools`,
`toolSnippets`, `promptGuidelines`, `appendSystemPrompt`, `contextFiles` and `skills`.
`toolSnippets` is a string-keyed ordered map. These are already-assembled inputs;
the guest does not discover resources or build the prompt.

| Event | Result | Properties of the result |
| --- | --- | --- |
| `resources_discover` | `ResourcesDiscoverResult` | `skillPaths?`, `promptPaths?`, `themePaths?` |
| `session_before_switch` | `SessionBeforeSwitchResult` | `cancel?` |
| `session_before_fork` | `SessionBeforeForkResult` | `cancel?`, `skipConversationRestore?` |
| `session_before_compact` | `SessionBeforeCompactResult` | `cancel?`, `compaction?` |
| `before_provider_request` | `BeforeProviderRequestEventResult` | a string holding the replacement request |
| `input` | `InputEventResult` | `action` is `continue`, `handled` or `transform`; a transform has `text` and `images?` |

`compaction` holds `summary`, `firstKeptEntryId`, `tokensBefore` and `details?`. A compaction
currently carries only the fields listed here. Session start, session shutdown and provider
responses have no result type delivered here; the added agent events also have
no result types delivered here, and the adapter does not check that a result belongs to the family
of the event it answers: the host reads it.

An `AbortSignal` is a capability, not a snapshot: keep it past the callback and read
`aborted()` later. Keeping the `ExtensionContext` works the same way. Both continue to read
the host resources lent for the invocation that delivered them, whatever later invocations
receive. What a handler does not keep is dropped when the invocation ends, including when
the delivery is refused.

## How an event travels

The guest exports one asynchronous function, `invoke-event`, for every event. The host passes
the callback, the event as one host-encoded JSON document whose `type` names the kind, and the resources
the event needs: a context, and for a compaction its signal. A signal that comes with any other
event is dropped. Resources never appear in the document.

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
may reject. An extension-assigned Infinity or NaN fails encoding with
`extension wrote a non-finite number (Infinity or NaN)`; the independent callback
failure or other encoded part is retained. Numeric-looking text remains text.

The request body of `before_provider_request`, its replacement, and the `details` of a
compaction are JSON text carried as a string. The adapter never parses or reformats it, so
duplicate keys, spacing, exponents and any nesting depth reach the handler as authored. A
replacement `null` is text and differs from returning no result; `details` text `null` differs
from `details` null.

The event's `type` tag must be a JSON string, not a single-property object. The reason fields
of resource discovery, session start, session switch and session shutdown, the fork position
and the input source accept only their declared JSON string literals in event records, not
object variants. They still serialize as lowercase strings.

The export decodes the host-encoded document before it enters the handler. The outer event,
`preparation` and input-image list elements are read from JSON objects. The outer tag reader
skips other properties, and each payload uses its owning record decoder. Shared records decode
through the same Serde derives used by the host, including their native record and union
admission; there is no additional validation of foreign input shapes or tolerance of
unrepresentable unread members. A decoding failure or a compaction without its signal makes
the export fail with an error message before handler entry. A callback identity that is unknown,
or registered for a command, does not fail the export: the outcome reports the lookup message
as the decision.

The outcome has two independent parts. `event` is the event as the handler left it, whether
it returned or failed, and is absent only when the handler replaced it with an event of another
kind, which includes another session event. `decision` is `returned` with the encoded result,
or `failed` with a message: the handler's own, or the lookup message. A failure to encode the event or the result is reported
in its own part, as a message, and never replaces the other part or the handler's failure.

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
tests in `src/tests/event_values.rs` and `src/tests/event_transport.rs` deliver the classes of
every property and every way an invocation can end through both hosts with the probe handler.
