# Writing an extension as a component

`maestro-extensions-wasm` is the Rust library an extension author links to. It owns the
canonical interface files in `crates/maestro-extensions-wasm/wit/` (package
`maestro:extension`) and a facade over the generated bindings, so an extension is ordinary
async Rust that the host runs as a WebAssembly component. The facade uses `maestro-request` for shared model, resource, application message, event/result and assembled prompt records. Native and guest aliases select their existing domain operands; the guest keeps its opaque text, header pairs and capability resources. Stream snapshots remain guest-adapter operations.
This page describes what is delivered so far: registration, 29 events and thirteen result
families, tool callbacks, command completions, shortcuts, flags, command contexts, session continuations, session readers,
event channels, live context observations, model catalog queries, ordinary actions, compaction notifications and custom message components.

Native waits, child processes, raw HTTP bodies and file watches are described in
[native operations](guest_operations.md).
Shared [model invocation options](guest_streams.md) keep native and guest data in one owner.

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
`register_command` forwards a command, `register_tool` forwards one tool definition and
`register_message_renderer` forwards a custom message renderer. A rejected registration returns the host's message and
adds no callbacks to the guest table.

Releasing a callback table entry removes that registration's ownership. Independent authored
aliases and admitted futures can retain its captures after release. Entries leave the table
before captures are destroyed, so a destructor may register another callback.

The native executor releases each invocation's admission operands before reporting completion,
including when a caller has stopped waiting. A component fault settles all started calls,
including callback releases, without entering guest code or running guest destructors again.
The Store remains alive until the last external admission ends; completed invocation records
do not keep it alive.

`CommandOptions` owns its handler and optional `get_argument_completions` callback.
Completion receives literal argument text and returns `None`, an empty list or an ordered
list of `AutocompleteItem` records. The guest forwards each value, label and optional
description without rewriting, sorting or deduplicating them. Command and completion
registrations release independently.

`register_shortcut` forwards the literal key and optional description in `ShortcutOptions`.
Its handler receives an ordinary [`ExtensionContext`](../../crates/maestro-extensions-wasm/src/types/context.rs),
not a command context. Immediate and suspended callback results retain supplied errors.

`register_flag` forwards `FlagOptions`: optional description, declared `FlagType` and an
independently typed optional `FlagValue` default. `get_flag` reads the host's current reply,
including absence, false and empty strings. The host owns default and override resolution;
the guest neither caches values nor infers the kind from the default.

### Environment completion example

```rust
use std::rc::Rc;
use maestro_extensions_wasm::{AutocompleteItem, CommandOptions, ExtensionAPI, ExtensionResult};

fn register_environment(api: &ExtensionAPI) -> ExtensionResult<()> {
    api.register_command("environment", CommandOptions {
        description: None,
        handler: Rc::new(|_, _| Box::pin(async { Ok(()) })),
        get_argument_completions: Some(Rc::new(|prefix| Box::pin(async move {
            let items = ["dev", "staging", "prod"].into_iter()
                .filter(|name| name.starts_with(&prefix))
                .map(|name| AutocompleteItem {
                    value: name.into(), label: name.into(), description: None,
                })
                .collect::<Vec<_>>();
            Ok((!items.is_empty()).then_some(items))
        }))),
    })
}
```

The example callback filters its own environment names; the adapter does not perform
completion matching.

## Tools

`ToolDefinition` combines `ToolMetadata`, optional synchronous `prepare_arguments` and
asynchronous `execute`. Preparation receives `&mut Value` and a callback-scoped
`CallbackEmitter`. In-place changes remain in the retained assistant arguments,
including when the callback fails. The returned value is separate: the tool caller
validates it before execution. The emitter forwards synchronous emission during
preparation. The adapter does not perform argument validation or schedule tools.
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
name predicates compare the name alone. Result input uses `JsonText`.
`UserBashEventResult` may independently return custom `BashOperations` and an
already-produced `BashResult`. Operations travel as a typed capability, outside the
[event/result document](#events-and-results).

```rust
use std::rc::Rc;
use maestro_extensions_wasm::{
    BashExecResult, BashOperations, ExtensionEventResult, Presence, UserBashEventResult,
};

api.on("user_bash", Rc::new(|_event, _ctx| Box::pin(async {
    Ok(Some(ExtensionEventResult::UserBash(UserBashEventResult {
        operations: Some(BashOperations {
            exec: Rc::new(|_command, _cwd, options| Box::pin(async move {
                (options.on_data)(b"custom output")?;
                Ok(BashExecResult { exit_code: None })
            })),
        }),
        result: Presence::Missing,
    })))
})))?;
```

The adapter forwards command, working directory, optional timeout in seconds,
optional environment values and raw byte chunks. It does not execute a process,
schedule a timeout, merge environments or sanitize output; those policies belong
to the supplied operation and its caller. Cloned operation, output and signal
owners may outlive the event or execution that supplied them. Callback errors
return to the author/caller without undoing prior output. For the signal's live
`aborted()` state, see [live context capabilities](#live-context-and-catalog-capabilities).

Registration and enumeration preserve nested parameter schemas and the kinds declared
for their nodes. An enumerated declaration can be registered again with those kinds
intact. Metadata transport does not validate invocation arguments; argument preparation
and validation remain with the model argument validator. Model-server JSON limits do
not apply to these author-supplied declarations.

### Tool renderers

`render_call` and `render_result` are optional and independent. Each returns a `Component`
that the host shows in the tool's row. A call renderer gets the arguments as they stand,
the live theme, a `ToolRenderContext` and a `CallbackEmitter`. A result renderer also gets
a `ToolRenderResultLike` and the current `ToolRenderResultOptions` (`expanded`, `is_partial`).
The rendering result contains the existing `AgentToolResult` in `result` and the independent
`is_error` observation. Execution results are unchanged.

`ToolRenderContext` holds the row's `tool_call_id`, arguments, working directory,
`ToolRenderPhase`, expansion, image visibility and error flag. `last_component` is the
component that the same renderer returned for the row last time. `invalidate()` asks the
host to redraw the row. `state::<S>()` returns the row's state of type `S`. It is created
with its default the first time it is read and shared by both renderers of the row on
every rendering, so a value set by the call renderer is visible to the result renderer.
The row keeps its state and previous components until the host releases the row.

A native factory also lends an opaque `native_row`. Delegating to its renderer preserves
the native environment and state while forwarding the wrapper's public row observations,
ordinary operands and exact previous component. See [native tool operations](guest_operations.md)
for read construction, replacement file callbacks and renderer delegation.

### Executable schema nodes

Use `ParameterSchema::with_code` to retain a `schema_code::SchemaCode` beside a
schema's JSON and declared kinds. Each pointer maps to the shared `SchemaNode`:
an optional `Base` and refinements in registration order. `Base::default()` leaves
values unchanged, accepts them and returns no guard errors. Its conversion can
return a replacement separately from edits to the supplied value. `None` keeps the
operand; `Some(Value::Null)` replaces a nested member with JSON null.

Guest `SchemaCallback` closures receive a borrowed `JsonView` and a
`CallbackEmitter`. `kind` and `read` distinguish a missing member from JSON null.
`read` and removed `splice` values admit the same nesting as tool metadata.
`member` borrows a child; `keys`, `set`, `delete` and `splice` expose ordered object
and array operations. An explicit `read` copies only the value the author asks
for. Starting a callback never serializes its subtree. Dropping a child view,
including on an error path, leaves it attached to its parent. Reassigning a local
view does not replace an argument slot.

Schema callbacks return the shared `DiagnosticErrorInfo`, preserving name,
message, stack and code. Ordinary extension operations still use `ExtensionResult`
and string errors. Schema code stays out of serialized metadata and provider
JSON; decoding ordinary JSON produces no callbacks.

### Foreign schema calls

Enumeration keeps executable schemas from this guest, other guests and native
callers. Bindings follow each tool, not the first tool in a listing. Each listing
retains independent identities; keeping the received schema or registering a new
tool with it keeps its callbacks alive. Releasing one identity does not revoke
another. The last holder releases the originating captures through the existing
native request pump and Store release path, not from a resource destructor.

`SchemaCallback::new` and `call` remain synchronous. Same-origin callbacks keep
their original capture identity and run locally, without host mediation. A foreign
call follows this path:

```text
SchemaCallback::call
  callback.schema-invoke → existing native import route
    scoped operand wait → retained native SchemaNode
      originating native callback or guest callback
  import-route.schema-result → original result or structured diagnostic
```

The native node lives in the receiving guest's existing capability registry. Its
scoped wait borrows the checker's original mutable value, including a selected
child, so edits remain visible on success and error without a subtree copy.
Passing an absent child to a foreign callback returns `JSON operand is absent`;
it never substitutes JSON null. A local guest callback can still observe absence
through `kind` and `read`.

The import route returns nested same-instance calls to the guest, which runs the
same Rust callback bodies its exports use. This supports A→B→A without entering
an instance while it is blocked inside an import. Only the dedicated Store thread
waits on synchronous host imports; the original native request receiver continues
servicing requests. No synchronous callback tries to suspend through an async
canonical import, and no additional registry or work queue is involved.

This relies on Wasmtime's blocking [`func_wrap` contract][schema-sync-import],
not on suspending a synchronous component task. Wasmtime [rejects such
suspension][schema-sync-task] and [prevents reentry into an active stackful
instance][schema-reentry]. These links name the pinned runtime revision. The host
continues to install [WASI 0.2 standard-library imports][schema-wasi] alongside its
native-async component exports; the foreign-call route requires no WASI polling
or new WASI interface.

The original facade's activity guard runs before entering a guest-origin schema.
Unload or reload leaves an already-admitted callback running to completion, but
later calls return that origin's stale-runtime message without guest entry.
Ordinary extension results and string-error APIs are unchanged.

[schema-sync-import]: https://github.com/bytecodealliance/wasmtime/blob/ffff2e2db24a291f02429f123aba1631fa8eea67/crates/wasmtime/src/runtime/component/linker.rs#L675-L683
[schema-sync-task]: https://github.com/bytecodealliance/wasmtime/blob/ffff2e2db24a291f02429f123aba1631fa8eea67/crates/wasmtime/src/runtime/component/concurrent.rs#L2728-L2736
[schema-reentry]: https://github.com/bytecodealliance/wasmtime/blob/ffff2e2db24a291f02429f123aba1631fa8eea67/crates/wasmtime/src/runtime/component/concurrent.rs#L784-L793
[schema-wasi]: https://github.com/bytecodealliance/wasmtime/blob/ffff2e2db24a291f02429f123aba1631fa8eea67/crates/wasi/src/p2/mod.rs#L314-L324

## Event channels

`api.events()` returns the supplied host's `EventBus`. Ordinary code awaits
`bus.emit(channel, data)`; a synchronous listener uses its borrowed `CallbackEmitter`.
Both forward literal channels and JSON payloads without trimming or sorting.
See [shared event-bus scheduling](event_bus.md) for the host's delivery rules.
The component adapter preserves the emission-time listener snapshot in registration order.
Same-instance listeners run locally, interleaved with synchronously acknowledged host segments;
nested emission takes a fresh snapshot. Each listener has its own callback-return and tail boundary.
The internal component exports are async-typed, but author callbacks and `CallbackEmitter::emit`
remain synchronous; no `.await` is added to author code.

`bus.on(channel, listener)` returns an explicitly invoked `Subscription`.
Dropping its facade does not unsubscribe; call `unsubscribe()` to forward removal.
Clones of bus and subscription facades share their supplied ports.

A listener's synchronous prefix returns `ExtensionResult<Option<ExtensionFuture<'static, ()>>>`:
`Ok(None)` has no tail, `Ok(Some(future))` carries an independently owned tail, and
`Err(message)` is forwarded to the host as the author's error. Failed listener lookup or payload
decoding returns a local diagnostic before listener invocation.
The adapter announces that future without polling it;
the host invokes the tail separately. Releasing the listener does not release a tail
already announced. The callback emitter cannot be retained by that tail: capture an
owned `EventBus` for later asynchronous emissions instead.

A component has no task of its own that outlives a callback. `Background::new(api.events())`
registers one listener that turns announced work into tails, and
`background.spawn(work).await` announces `work` and returns its `BackgroundTask`. The host
polls the work on that tail until the work ends; dropping the task stops it, dropping any
pending wait, and settles the tail. A refused announcement fails the start, and the work
never runs. A failure of the work goes to the host's event-bus reporter.

## Session readers

Ordinary and command contexts expose `session_manager()`, which acquires the currently
bound `ReadonlySessionManager` on each call. Its thirteen queries forward the host's
strings, optional records and ordered lists without guest-side trimming, sorting or
path resolution. Entry records use [the existing data boundary](#events-and-results).
The `data` in custom entries and `details` in compaction, branch-summary and custom-message
entries use `JsonText`. It holds compact JSON text while the wire carries the JSON value,
including objects and arrays. It preserves key order and number values; missing and null
remain distinct. Borrow it as `&str`, or convert owned text with `.into()`. Authored text
must be valid JSON when serialized. The same conversion applies to entries in events.

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
| `tool_result` | `ToolResultEvent` | `toolCallId`, `toolName`, opaque `input`, shared `content`, `isError`, retained JSON `details?`, shared `usage?` |
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
Model selection carries the selected and previous descriptors with their open
compatibility objects, including mixed-family and custom members. Handler edits to
the selected object return independently of the previous descriptor. See
[shared records](../records.md#wire-records-and-helpers) for construction and ownership.
Reasoning selection carries the current and previous levels without checking model support.

`BuildSystemPromptOptions` carries `cwd` and optional `customPrompt`, `selectedTools`,
`toolSnippets`, `promptGuidelines`, `appendSystemPrompt`, `contextFiles` and `skills`.
`toolSnippets` is a string-keyed ordered map. These are already-assembled inputs;
the guest does not discover resources or build the prompt.

| Event | Result | Properties of the result |
| --- | --- | --- |
| `tool_call` | `ToolCallEventResult` | `block?`, `reason?` |
| `tool_result` | `ToolResultEventResult` | `content?`, opaque `details?`, `isError?`, `usage?` |
| `user_bash` | `UserBashEventResult` | custom `operations?`, supplied `result?` |
| `context` | `ContextEventResult` | `messages?` |
| `message_end` | `MessageEndEventResult` | `message?` |
| `before_agent_start` | `BeforeAgentStartEventResult` | `message?`, `systemPrompt?` |
| `resources_discover` | `ResourcesDiscoverResult` | `skillPaths?`, `promptPaths?`, `themePaths?` |
| `session_before_switch` | `SessionBeforeSwitchResult` | `cancel?` |
| `session_before_fork` | `SessionBeforeForkResult` | `cancel?`, `skipConversationRestore?` |
| `session_before_compact` | `SessionBeforeCompactResult` | `cancel?`, `compaction?` |
| `session_before_tree` | `SessionBeforeTreeResult` | `cancel?`, `summary?`, `customInstructions?`, `replaceInstructions?`, `label?` |
| `before_provider_request` | `BeforeProviderRequestEventResult` | the replacement JSON value, held as `JsonText` |
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

## Live context and catalog capabilities

`ExtensionContext` forwards `ui`, `has_ui`, `model_registry`, `model`, `is_idle`,
`signal`, `abort`, `has_pending_messages`, `shutdown`, `get_context_usage` and
`get_system_prompt` to its bound host context on each call. Command and replacement
contexts inherit these methods. Getter and action failures are recoverable
`ExtensionResult` errors; the guest does not implement host shutdown or queue policy.

`AbortSignal::subscribe(listener)` registers a listener the host runs once, on the
thread that owns the extension, when the signal is aborted. An abort that comes before the
subscription is not replayed, so read `aborted()` as well. The returned `Subscription`
removes the listener when you call `unsubscribe()`; dropping it does not. The listener
receives a `CallbackEmitter` and may subscribe or unsubscribe other listeners while it
runs.

`ExtensionUIContext`, `ModelRegistry` and `AbortSignal` retain their acquired host
resource independently of the context. Cloning shares the owner; its final alias
drops that resource. A later acquisition can return a different resource. Whether
an acquisition or retained query is allowed remains the host's decision.

`ModelRegistry::get_all`, `get_available` and `find` forward the host's results;
`find` passes literal provider and model identifiers. The guest does not filter,
sort or deduplicate the supplied lists. `get_api_key_and_headers` awaits the host
operation for the supplied descriptor, without checking catalog membership.
`ResolvedRequestAuth` is re-exported from the [shared request owner](../records.md#resolved-request-authentication),
`maestro_request::model_registry`. `ResolvedRequestAuth::Ok` preserves optional keys and ordered headers, including
present empty strings; `Error` carries a structured denial, distinct from an
outer host-call failure.

`ModelRegistry` also forwards `refresh`, `get_error`, `has_configured_auth`,
`get_provider_auth_status`, `get_provider_display_name`, `is_using_oauth`,
`register_provider`, `unregister_provider` and `get_api_key_for_provider` to the
same host catalog. Models and provider data are checked for nonfinite numbers
before the call; `AuthStatus` and `AuthSource` are the
[shared credential records](../records.md#credential-records).

`ModelRegistry::auth_storage()` returns the catalog's live credential owner, not
copied credentials. Its `AuthStorage` handle can outlive the creating callback.
Reads, runtime key overrides, stored credential changes, reload, logout and error
draining reach that same owner. `get_api_key` uses its original credential-command
effects; `GetApiKeyOptions::include_fallback` defaults to true. Persistence failures
remain in `drain_errors` rather than rolling back accepted state.

A stale registration facade does not invalidate credentials acquired earlier.
Malformed host input and nonfinite authored numbers fail before provider or
credential mutation; another valid resource remains callable. Rejections preserve
the host's supplied error text.

The guest currently acquires credentials from a catalog or wraps a supplied
`AuthStoragePort`. `AuthStorage::create(&api, path)` and
`ModelRegistry::create(&api, auth_storage, path)` accept independent optional file
paths. Omitted paths use lazy defaults supplied by the application before factory
loading. Both `in_memory` constructors avoid file access; catalogs retain the
supplied credential owner rather than copying its current records.

`AuthStorage::from_storage(&api, backend)` retains an authored `AuthStorageBackend`.
Its synchronous and awaited methods receive a borrowed native update. `None`
leaves stored text unchanged; `Some("")` writes empty text. The awaited update can
remain pending without blocking the host's local executor. Backend failures keep
the native accepted-state and drainable-error behavior.

`set_fallback_resolver` retains the supplied callback until replacement or final
release. It returns absent, empty and nonempty keys distinctly and preserves the
callback's error text. Backend and fallback callbacks may call back into the same
host, including from synchronous credential and catalog metadata reads. Callback
resources stay typed; their identities are never placed in JSON. Callable provider
stream and OAuth fields remain separate from the provider data record.

### Native OAuth providers

`AuthStorage::get_oauth_providers()` returns callable native implementations in
registration order. Each retained `OAuthProviderInterface` exposes its literal
`id`, login `name` and optional `uses_callback_server` value. An absent value is
not changed to false. Removing a provider from the native registry does not
replace an already retained implementation; its final guest alias releases it.

`get_api_key` and `modify_models` are synchronous. `refresh_token` awaits the
native token exchange without adding an expiry decision or persisting the
returned credentials. Its future retains the provider after the initiating
handler and facade are released. `modify_models` returns `None` when the provider has no
modifier. The guest forwards complete credentials and model descriptors,
including open credential fields, and rejects authored nonfinite numbers before
encoding them. Provider failures retain their messages.

`login` returns an owned future over the same native provider. Supply
`Rc<OAuthLoginCallbacks>` with synchronous `on_auth` and optional `on_progress`
notifications, an awaited `on_prompt`, and optional awaited manual-code and
selection callbacks. An absent selection callback differs from a supplied
callback returning `None` on cancel. Prompt records keep supplied text, optional
fields and choice order. Callback failures retain their messages, and synchronous
notifications may call back into the host before returning.

The optional caller signal follows live cancellation, including a prior abort.
Native callback aliases retain the interaction and its signal binding after the
initiating login future ends; the final native release drops their captures.
Pending input does not block other host requests. These methods reuse the native
OAuth implementations; provider methods do not store login credentials.

`AuthStorage::login(provider_id, callbacks)` instead delegates authorization and
persistence to the retained native credential owner. It returns an owned future,
so dropping the initiating facade does not end the operation. Successful login
accepts the complete OAuth record and persists it through the original backend;
other provider records stay unchanged. Unknown providers and interaction failures
return their original messages. As with `AuthStorage::set`, persistence failures
leave accepted state available and are reported by `drain_errors`, not as a failed
login.

`model` and the catalog queries reuse the [shared model records](../records.md).
`ContextUsage` carries optional token and percentage estimates and the supplied
floating-point context window without recalculation. No usage is distinct from
usage whose two estimates are unknown.

## Retained UI requests

[Guest UI](guest_ui.md) describes `ExtensionUIContext` requests, the widget, header and
footer factories, themes, terminal input and dialogs, and lists each UI resource with
its native counterpart.

The component example's controlled compaction handler reads the supplied preparation
and branch entries, queries the session reader and catalog, and returns compaction
content whose opaque details depend on those live query results. Summary formatting
remains in its existing author helper.

## Ordinary actions

`ExtensionAPI` provides `send_message`, `send_user_message`, `append_entry`,
`set_session_name`, `get_session_name` and `set_label`. Ordinary sends forward the
supplied data and optional delivery choices to the host. `SendMessageOptions`
requests `steer`, `followUp` or `nextTurn` delivery and an optional `trigger_turn`;
`SendUserMessageOptions` requests `steer` or `followUp`. Missing options and empty
options remain distinct. Clearing a label uses `None`, not an empty string.

`get_active_tools`, `get_all_tools` and `get_commands` return the host's ordered
records without sorting or deduplication. `set_active_tools` forwards the requested
names. [`ToolInfo`](../../crates/maestro-extensions-wasm/src/types/actions.rs) and
[`SlashCommandInfo`](../../crates/maestro-extensions-wasm/src/slash_commands.rs)
retain host-supplied provenance. See the [data boundary](#how-an-event-travels) for
schema and details text with semantic JSON serialization.

## Provider registration

`ExtensionAPI::register_provider` sends a literal name and a `ProviderConfig` to the
host, and `unregister_provider` sends a name. `ProviderConfig` and
`ProviderModelConfig` use the [shared provider records](../records.md#provider-registration-records).
`ProviderConfig` specializes its callback slot with the guest's `ProviderCallbacks`.
Serialized metadata carries data only. Set `callbacks.oauth` to `Some(Rc<dyn OAuthProviderInterface>)` to register
an authored OAuth implementation through the same API or catalog operation. The
registration name supplies its installed identifier; its login label and optional
callback-server value remain as authored. The native catalog retains the actual
guest implementation, including independently owned login and refresh futures,
synchronous computed keys and optional model transformation. Login receives the
original native interaction through retained capabilities, preserving callback
presence, synchronous notifications, awaited input and live cancellation. The guest
may keep the callback record or an input future after login ends; its last alias
releases the original native captures. Pending input leaves the host available,
and synchronous native notifications may reenter the same guest implementation.

Absent fields are omitted; empty strings, lists and maps and
`false` are sent as written, header, model and input order is kept, and signed zero
survives. A nonfinite number fails with the finite-number error before the host
sees the call. Queuing during loading, validation and replacement stay host policy.

## Model selection and action errors

`set_model` waits for the host's boolean reply; other ordinary actions return their
immediate host result. `false` is a successful selection reply, not a guest error.
`get_thinking_level` reads the supplied level and `set_thinking_level` forwards the
requested level. Authentication, catalog construction, clamping, queues and
persistence remain host policy. Host rejections, including loading and stale-context
messages, remain recoverable `ExtensionResult` errors (inside the future for model
selection).

## Explicit execution

`ExtensionAPI::exec(command, args, options)` runs a program with ordered literal
arguments. No shell is involved, so quotes, globs, `$` and `;` reach the program
unchanged, and stdin is closed. The future resolves to an `ExecResult` with separate
`stdout` and `stderr` text, the exit `code` and `killed`. A program that cannot start
yields empty streams and code 1. A program ended by a signal it did not get from the
caller has code 0 and `killed` false. An empty command or an operand the operating system
rejects is an error, as are host and stale-context rejections.

`ExecOptions` has three optional fields:

| Field | Effect |
| --- | --- |
| `cwd` | Working directory. Absent uses the directory the extension was loaded with; an empty string uses the host process directory. |
| `timeout` | Milliseconds before termination. Zero or a negative value sets no deadline; a value too large for a native duration never expires, and a positive value below one nanosecond expires at once. |
| `signal` | Any `AbortSignal`, including one backed by your own `SignalPort`. An abort before or during the run terminates the program and sets `killed`. |

The host keeps validation, process supervision and termination escalation: a
program that ignores termination is killed five seconds later. The guest bridges a
supplied signal to the host for each run and removes its listener when the run
settles, whatever the outcome, or when the run's future is dropped, so other
listeners on the same signal and later runs keep working. Returned shell operations
can pass their supplied signal to `exec` and still observe an abort after their event
handler returned.

## Compaction admission and notifications

`ExtensionContext::compact` is inherited by command and replacement command views.
It returns admission before its optional completion or error notification.
[`CompactOptions`](../../crates/maestro-extensions-wasm/src/types/context.rs) carries
optional literal instructions, `on_complete` and `on_error`. Absent options remain
distinct from present options with every field empty. Completion receives
`CompactionResult`; error receives the supplied message. A callback failure returns
to the host, which decides whether to invoke the error callback.

Accepted notifications retain their callback entries until host release; rejected
admission retains neither entry. Host release removes a callback's table ownership;
separately retained aliases keep their captures. Compaction notifications receive
the existing callback-scoped emitter: see [event channels](#event-channels). The
guest forwards emissions; the host routes them.

## How an event travels

The guest exports one asynchronous function, `invoke-event`, for every event. The host passes
the callback, the event as one host-encoded JSON document whose `type` names the kind, and the resources
the event needs: a context, and for before-compaction or before-tree its owned signal.
A signal that comes with any other event is dropped. Resources never appear in the document.

Unshared optional event fields keep three states apart: omitted, explicitly `null`, and present, including
empty strings, empty lists and `false`. Such an event field is a
`Presence<T>`: it defaults to `Missing` when the property is absent and is skipped on
serialization only while it is `Missing`. Serializing `Missing` on its own, outside a property,
is an error; `Null` serializes as `null` and `Present` as its value.

Shared model records retain their own optional-field serialization: ordinary `Option` fields
collapse missing and null, while existing nested options retain explicit null.
[Application message records](../records.md#application-message-records) keep their existing
`Presence` fields; model records have no added wrapper. The inventory record
[`SlashCommandInfo`](../../crates/maestro-extensions-wasm/src/slash_commands.rs)
also uses standard `Option` decoding for `description`: missing and null both become `None`
and are omitted on serialization; an empty string is retained. Command source is an open
string with three default labels, not a closed enum. The declaration belongs to the shared
request owner; see [ordinary command metadata](../application/commands.md).

Numbers use plain JSON with exact finite floating-point parsing, including signed zero.
Ordinary host nonfinite serialization writes null, which a required numeric decoder
may reject. `ToolInput::from_value` rejects Infinity or NaN before returning a carrier.
Other extension-assigned Infinity or NaN fails encoding with
`extension wrote a non-finite number (Infinity or NaN)`; the independent callback
failure or other encoded part is retained. Numeric-looking text remains text.

The request body of `before_provider_request`, its replacement, tool-execution arguments
and results, tool-result input and arbitrary `details` use `JsonText`.
It exposes compact JSON text to Rust while serializing the semantic JSON value on the wire.
Tool schemas are `maestro_request::parameter_schema::ParameterSchema` values inside the
tool metadata, not quoted JSON documents. Thinking levels
cross the component interface as JSON string literals in both directions.
Object key order and numbers are preserved; whitespace and duplicate keys are not retained.
Construct authored values from valid JSON text with `.into()`. A replacement `null` differs
from returning no result. Optional details retain missing and null as distinct states.

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

The guest re-exports its session, compaction preparation and result records from
[the entry owner](../entry.md), specializing messages and opaque detail text.
Every entry carries `id`, nullable `parentId` and a string `timestamp`; the guest
interprets neither IDs nor timestamps. Typed null parent/leaf/ancestor IDs serialize
as present null, unlike omitted `Presence` fields. Their `Option` decoder also accepts
absence. The tagged `SessionEntry` variants carry these additional fields:

| `type` | Fields |
| --- | --- |
| `message` | `message` (shared or application message) |
| `thinking_level_change` | `thinkingLevel` (open text) |
| `model_change` | `provider`, `modelId` |
| `compaction` | `summary`, `firstKeptEntryId`, `tokensBefore`, `details?`, `usage?`, `fromHook?` |
| `branch_summary` | `fromId`, `summary`, `details?`, `usage?`, `fromHook?` |
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
an `ExtensionCommandContext`, which adds `wait_for_idle`, `new_session`, `fork`,
`navigate_tree`, `switch_session` and `reload`. The ordinary context has none of these
methods, which the rustdoc examples check at compile time.

`new_session` accepts optional `setup` and `with_session` callbacks; `fork` and
`switch_session` accept an optional `with_session`. Absent options remain distinct
from present empty options, even when no callback is supplied. Tree navigation and
reload have no replacement continuation. Setup receives a
writable [`SessionManager`](../../crates/maestro-extensions-wasm/src/session_manager.rs);
ordinary contexts still expose a reader. When the host invokes the continuation, it
supplies a `ReplacedSessionContext` while the operation is pending. The
host calls the continuation closure once. Captures the closure keeps drop when that call
returns; captures moved into the returned future drop when the future completes. The identity
the host announced for it lasts until the operation ends, and a continuation the host never
ran is released then. The facade forwards the host's answers: after the operation, `cwd` on the
context that started it returns whatever the host reports, which the test host makes a
stale-context message. A replacement context retains its host capability when cloned;
the host can still reject later calls. Its `send_message` and `send_user_message`
methods await that capability's host send, unlike the ordinary
[message actions](#ordinary-actions). Errors are the plain text the host supplies,
so an author can catch one and continue.

The writer exposes `set_session_file`, `new_session`, `is_persisted`, `get_children`,
`build_session_context`, the message/thinking/model/compaction/custom/name/label append
operations, `branch`, `reset_leaf`, `branch_with_summary` and `create_branched_session`.
Writer operations delegate to the host; required encoding and selected-response decoding
can fail locally. `NewSessionOptions` carries optional writer-chosen `id` and
`parent_session` strings, separately from command options. `SessionContext` contains the
host's ordered messages, open thinking-level string and optional provider/model identity.

Setup entries belong to their new-session operation; continuation entries belong to
their new-session, fork or switch operation. These entries are removed
when their owning future ends or is dropped. Setup's writer and its reader address the capability
supplied for that invocation; independently retained capability handles keep their own
owners. The guest implements no persistence, context projection or navigation policy.

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


### A greeting tool

[`examples/extensions/hello.rs`](../../examples/extensions/hello.rs) is the
shipped greeting extension: it registers one `hello` tool whose schema requires
the string member `name`, and answers with `Hello, <name>!` while returning the
same name as `greeted` details. It keeps the name byte-for-byte, writes no
progress updates and leaves optional metadata and termination absent.

Loading it into an application requires a host that enables the component
runtime, so the extension stays opt-in. The extension crate's tests instantiate
the built component in a controlled host; `examples/extensions/README.md` gives
the build and controlled-run commands.

### A shutdown command and two tools

[`examples/extensions/shutdown_command.rs`](../../examples/extensions/shutdown_command.rs)
is the shipped shutdown extension. Its `/quit` command requests shutdown as soon
as it runs, whatever arguments it receives. The `finish_and_exit` tool requests
shutdown first and then returns `Shutdown requested. Exiting after this
response.`. With a progress handle, `deploy_and_exit` reports `Deploying to
<environment>...` and `Deployment complete, exiting...`, requests shutdown and
returns `Done! Shutdown requested.` with the environment as its details. The
deployment tool is illustrative and runs no deployment process.

The three callbacks read no cancellation signal and write no console output. A
refused registration stops the ones that follow, and a refused shutdown or
progress update leaves the call with that message instead of a result. Loading
the component into an interactive host is opt-in and asks that host to shut
down; `examples/extensions/README.md` gives the build and controlled-run
commands.

### Tool-aware prompt guidance

`examples/extensions/prompt_customizer.rs` is the shipped prompt-guidance
extension. It handles `before_agent_start` and returns a replacement
`systemPrompt`: the supplied prompt, the guidance for each active built-in tool
in read, bash, edit and write order, the supplied skill names in their original
order, and this extension's own context section.

It reads only the assembled options, so it discovers no resources and builds no
prompt. A supplied user append is already part of the event's `systemPrompt`,
including any edit an earlier handler made; the example never appends that
option again. Loading it into an application is opt-in, and
`examples/extensions/README.md` gives the build and controlled-run commands.

### A hidden thinking label

[`examples/extensions/hidden_thinking_label.rs`](../../examples/extensions/hidden_thinking_label.rs)
is the shipped collapsed-thinking-label extension. Its `/thinking-label`
command sets the label the host shows, and the same command without arguments
clears that label and restores `Pondering...` for the next session start. The
example also registers a `session_start` handler that applies the label it
retains, so the next session shows the label the session before set.

The command trims the surrounding whitespace of the text it receives and keeps
the interior text unchanged, and a command that sends no text takes the reset
branch rather than setting an empty label. Clearing the label and applying the
example default are separate requests: the first asks the host to restore its own
default, and the second shows the example's text at the next session start.

A host rejection ends the command with the host's message. The command has
already stored the label it chose, so the next session start applies it. Loading the component into an application is opt-in and needs a host that
enables the component runtime; `examples/extensions/README.md` gives the build
and controlled-run commands.

### A prompt toggle

`examples/extensions/pirate.rs` registers a `/pirate` command and a
`before_agent_start` handler that share one boolean the factory instance owns.
The command flips that boolean and reports the new state through
`ExtensionUIContext::notify`; the handler appends this example's instructions to
the supplied `systemPrompt` only while the boolean is set and otherwise returns
no override. A refused notification leaves the flip in place. Loading it into an
application is opt-in, and `examples/extensions/README.md` gives the build and
controlled-run commands.

### A system prompt status header

[`examples/extensions/system_prompt_header.rs`](../../examples/extensions/system_prompt_header.rs)
registers `agent_start` to read the prompt the host supplies at that moment and
set the `system-prompt` status to `System: N chars`, where `N` counts UTF-16 code
units of that prompt. Whitespace, a byte order mark and combining sequences keep
the width the string stores, and nothing is trimmed, rebuilt or cached.
`session_shutdown` removes the status under the same key, including when no start
came first, so the handler keeps no state between invocations.

A host rejection from the getter or the status request reaches the host
unchanged, and no fallback status is written. Loading the component into an
application is opt-in, and `examples/extensions/README.md` gives the build and
controlled-run commands.

### A duration tool with its own dependency

`examples/extensions/with-deps/main.rs` is the shipped example that parses
durations with a regular expression. The dependency is the workspace pin,
declared once as a development dependency of the example crate, so its examples
and tests share it and the guest library never depends on a parsing library. It
registers one `parse_duration` tool whose schema requires the string member
`duration`, returns `<duration> = <milliseconds> milliseconds`, and leaves the
details an empty document with optional metadata and termination absent.

Units run from milliseconds to years, each with its short and long aliases and
matched case-insensitively over ASCII. A bare number means milliseconds, and a
leading minus sign or decimal point is allowed. The number is rendered as the
shortest spelling of the double it rounds to, so values below a nanosecond keep
exponent notation.

Text outside the grammar, or longer than 100 bytes, is rejected as
`Invalid duration: "<text>"`, with the original text inside the message; the
empty text returns the dependency's own message instead.

Loading it into an application requires a host that enables the component
runtime, so the extension stays opt-in. The extension crate's tests instantiate
the built component in a controlled host and compare every recorded duration
text; `examples/extensions/README.md` gives the build and controlled-run
commands.

### A runtime reload command

[`examples/extensions/reload_runtime.rs`](../../examples/extensions/reload_runtime.rs)
registers a `reload-runtime` command whose handler is its awaited
`ExtensionCommandContext::reload` request, so the command ends when the host
finishes reloading. The same factory registers a `reload_runtime` tool that
queues `/reload-runtime` as a follow-up user message and then answers
`Queued /reload-runtime as a follow-up command.` with present empty details.

The tool does not reload on its own. Tools run with an `ExtensionContext`, which
has no reload method, so the queued follow-up is how a model-triggered tool
reaches a command-only operation. The rustdoc example on the ordinary context
rejects that call at compile time.

The example is opt-in like the greeting tool: its crate's tests run the built
component in a controlled host, and `examples/extensions/README.md` holds the
build and controlled-run commands. Reloading a running application is not part
of that controlled run.

### An input transform hook

[`examples/extensions/input_transform.rs`](../../examples/extensions/input_transform.rs)
registers `input` and bypasses extension-injected messages. It rewrites a nonempty
`?quick <question>` as a brief-answer request, warns and handles an empty question,
and handles `ping` and `time` with local notifications. Every other input returns
an explicit continuation. Rewrites omit replacement images and leave the event
unchanged.

The example is opt-in and needs a component-enabled host with notification UI.
The [example index](../../examples/extensions/README.md) gives its build and
controlled-run commands and the time-format limit.

### A protected-paths tool-call hook

[`examples/extensions/protected_paths.rs`](../../examples/extensions/protected_paths.rs)
handles `tool_call` and returns a blocking `ToolCallEventResult` when a `write`
or `edit` call names a path containing `.env`, `.git/` or `node_modules/`. The
reason is `Path "<path>" is protected`, and the hook first asks the context
whether the host has UI: if it does, it sends one warning,
`Blocked write to protected path: <path>`, before returning the verdict.

It reads the path after the tool-name filter, so every other tool passes
through untouched and an unreadable path operand never reaches the UI. Matching is
a case-sensitive literal substring test on the authored path: dot segments,
uppercase spellings and backslash separators are left as they are, and no
filesystem call decides the answer. The three patterns are the example's own
policy; a hook needs a host that enables the component runtime, and
`examples/extensions/README.md` holds the build and controlled-run commands.

### A compaction trigger

[`examples/extensions/trigger_compact.rs`](../../examples/extensions/trigger_compact.rs)
registers a `turn_end` hook and a `trigger-compact` command. The hook records the
context usage the host reports and requests compaction once a known estimate at or
below 100,000 tokens is followed by a known estimate above it; an estimate the host
does not report leaves the last one in place. The command requests compaction at
once, passing its trimmed arguments as the instructions, or none when they are
blank, and it never changes the recorded estimate.

The example only asks the host for compaction and reports `Compaction started`
before the request, then `Compaction completed` or `Compaction failed: <message>`
when the host answers. Each report reads the UI availability of that moment, so a
headless start can still report a completion that arrives after a UI appears.

It is opt-in like the greeting tool: its crate's tests run the built component in a
controlled host, and `examples/extensions/README.md` holds the build and
controlled-run commands. Compacting a running application is not part of that
controlled run.

### Entry bookmarks

[`examples/extensions/bookmark.rs`](../../examples/extensions/bookmark.rs) labels
session entries, which is what the tree view shows beside an entry. `/bookmark`
labels the newest assistant message: with the argument text when it has any, and
with `bookmark-<epoch milliseconds>` when the argument is blank. `/unbookmark`
removes the newest label from any entry kind and ignores its arguments.

The example reads the complete entry list once per invocation, walks it from the
newest entry and stops at the first match, so a bookmark never scans the list
twice. It keeps no label store of its own: `set_label` and `get_label` reach the
host's session, and a label it removes keeps the entry it was written on.

Entry identity comes from the entry `message`, `thinking_level_change`,
`model_change`, `compaction`, `branch_summary`, `custom`, `custom_message`,
`label` and `session_info` all carry, so removal covers every kind. Both commands
return host failures unchanged, and a command that writes a label before
notifying writes nothing when the label write itself fails.

It is opt-in like the other shipped extensions: its crate's tests run the built
component in a controlled host with a supplied wall clock, and
`examples/extensions/README.md` holds the build and controlled-run commands.

### Dynamic tools

[`examples/extensions/dynamic_tools.rs`](../../examples/extensions/dynamic_tools.rs)
sets up an `echo_session` tool from its `session_start` handler, and adds
`/add-echo-tool <tool_name>` as a command that registers more echo tools after
initialization. The argument is trimmed and lowercased with the standard
library's whitespace and case rules, then admitted only when what remains is
nonempty ASCII lowercase letters, digits and underscores. Anything else answers
`Usage: /add-echo-tool <tool_name> (lowercase, numbers, underscores)`.

Each admitted name becomes an `Echo <name>` tool whose description names its
prefix, and whose execution answers `<prefix><message>` with the registered name
and prefix as details. The factory keeps one set of admitted names, inserts a
name before asking the host to register it, and keeps that name when the request
fails. A refused request admits no tool, so a retry reports
`Tool already registered: <name>` without asking the host again. It never asks
whether the host reports UI availability before requesting a notification.

It is opt-in like the other shipped extensions: its crate's tests replay the
recorded sequences through the built component in a controlled host, and
`examples/extensions/README.md` holds the build and controlled-run commands.

### A custom working message on session start

[`examples/extensions/working_message_test.rs`](../../examples/extensions/working_message_test.rs)
registers one `session_start` handler. It acquires the context's UI and sends
`set_working_message` with the authored text, then `set_working_indicator` with
one authored frame and no interval, and returns no event result. Both requests
keep every authored byte, including the colour and reset sequences. A refusal the
host returns ends the callback with the host's message, so a refused working-text
request leaves the indicator unsent.

The controlled run asserts the ordered requests the guest sends; it does not show
that an interactive host keeps them across agent turns or loader recreation, which
belongs to the host's own working-loader operations. Loading the component into an
application is opt-in and needs that application's component runtime.

### Widgets above and below the editor

[`examples/extensions/widget_placement.rs`](../../examples/extensions/widget_placement.rs)
handles `session_start`. It reads `has_ui` once per start and requests nothing
when the host has no UI. With a UI it requests the `widget-above` widget holding
the single line `Above editor widget`, then the `widget-below` widget holding
`Below editor widget` with the explicit below placement. The first request
passes no options, so the host's own above-editor default applies.

The callback acquires the UI context once per request, so a host that binds a
different UI after the first accepted request serves the second one there. A
rejected request ends the callback with that message; the example retries
nothing and removes no widget the host already accepted. Loading it into an
application is opt-in, and `examples/extensions/README.md` holds the build and
controlled-run commands.

### User message delivery

[`examples/extensions/send_user_message.rs`](../../examples/extensions/send_user_message.rs)
sends user messages the way a user types them. `/ask` sends an ordinary message,
which starts a turn, and reports `Agent is busy. Use /steer or /followup
instead.` while the agent is working. `/steer` sends an ordinary message while
idle and a steering message while the agent is working, so that message reaches
the next model request. `/followup` sends an ordinary message while idle and
otherwise queues a follow-up, reporting `Follow-up queued` once the host admitted
it. `/askwith` sends two text blocks, `User request: <arguments>` and `Please
respond concisely.`, so the second instruction arrives with the request.

A command with no message reports its own usage warning before it reads the idle
flag or sends anything: `Usage: /ask <message>` and its three siblings. Message
text is sent exactly as it arrived, whitespace included, even
though the same text decides whether the argument is blank. Each command returns
a host failure unchanged, and a queued follow-up acknowledges only a send the host
admitted.

It is opt-in like the other shipped extensions: its crate's tests run the built
component in a controlled host and `examples/extensions/README.md` holds the build
and controlled-run commands. Delivering those requests from a running
application is part of the queue the application owns, not of this controlled run.

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
                        usage: Presence::Missing,
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

## Custom message components

`register_message_renderer` forwards the literal custom type and its `MessageRenderer` to
the host. The callback receives a complete `CustomMessage`, `MessageRenderOptions.expanded`,
an owned opaque `Theme` and a borrowed `CallbackEmitter`. See [the data boundary](#events-and-results)
for shared records and [event channels](#event-channels) for emission.

Returning `None` leaves fallback selection to the host; returning a `Component` retains
its authored `ComponentView`. Cloned components share that view. Its synchronous `render`,
`handle_input`, `wants_key_release` and `invalidate` methods forward to the retained view;
input defaults to a no-op and the release preference defaults to false. Rendering receives
the supplied width; successful rendering returns the author's ordered lines. Returned errors remain errors.

Cloned themes retain the supplied host owner independently of renderer registration.
Each `Theme` method sends one synchronous request to that owner. The owner keeps the palette
and the style algorithms, so every call reads its current state:

| Method | Answer |
| --- | --- |
| `name`, `source_path` | The theme name and file path, or `None`. |
| `source_info`, `set_source_info` | Reads, replaces or clears the theme's `SourceInfo`. |
| `fg`, `bg` | Text wrapped in a named foreground or background color that restores only that color. |
| `bold`, `italic`, `underline`, `inverse`, `strikethrough` | Text with that one style and its own closing escape. |
| `get_fg_ansi`, `get_bg_ansi` | The escape that selects a named color. |
| `get_color_mode` | The `ColorMode`: `Truecolor` or `Color256`. |

The styled text is the owner's bytes, unchanged. An unknown color name is an error carrying the
owner's message. A theme kept past its renderer call observes later theme changes and provenance
edits made through any of its clones.

See [native helpers](guest_operations.md) for synchronous terminal operations.

## Standard operating-system interfaces

`tools::path_utils::make_temp_dir(&api, prefix)` creates a distinct directory
under the host's temporary root, with mode `0700` on Unix and the default
Windows ACL. The prefix must be a nonempty leaf name; the caller owns cleanup.

Rust standard-library environment, file and byte I/O use the host's WASI 0.2
baseline. `maestro_extensions_wasm::wasi` also exposes the pinned standard WASI
0.3 asynchronous interfaces, including sockets, name lookup, filesystem streams
and `wit_stream` byte-stream construction. Both generations use the same host
context; neither introduces an extension-specific permission list. See
[component operating environment](host_io.md) for access and platform limits.
