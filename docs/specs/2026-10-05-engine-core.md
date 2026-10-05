# Spec: engine core

## Problem Statement

Users need the same conversation, tool execution and saved history whether they
chat, run a script or embed Maestro. Changing a model, store or extension must not
require changing those callers. Interruptions must leave understandable results,
not promises that external effects were undone.

## Solution

Build a native Rust engine around one application interface and one active
session per runtime. The executable owns one runtime. Supply the agent loop,
native tools, branchable sessions, compaction, settings, credentials, extension
programs, packages and instruction resources. Offer plain chat, final-text print,
JSON events, JSONL RPC and Rust embedding through the same application.

The [models specification](2026-10-04-models.md), as narrowed by
[the model-access amendment](https://github.com/Orchestration-Maestro/maestro/issues/23),
owns model types, operations, events, request authentication and provider defaults.
This specification consumes that contract rather than defining another model
interface. Both reviewed specifications must land before their implementation.

Sessions use a replaceable storage seam. The core supplies an in-memory adapter
for tests and explicit no-session runs. Base Maestro wires SQLite as the default
persistent store; its implementation receives a separate short specification.
MCP ships in a separate base package, not in this core. Restartable jobs also
receive a separate specification. Neither is a prerequisite for testing the core.

Settings locks, declared native extension programs, the governed manifest and
`maestro init` remain deliberate Maestro capabilities. Keep one current format;
add no compatibility reader or migration machinery.

## User Stories

### Operate one application
1. **US01.** As a user, I want plain terminal chat, so that I need no full-screen interface.
2. **US02.** As a script author, I want final text or JSON events, so that output needs no terminal scraping.
3. **US03.** As a client author, I want correlated RPC requests, so that acceptance is distinct from execution outcomes.
4. **US04.** As an embedder, I want an injectable Rust application interface, so that I need not launch a frontend.
5. **US05.** As a user, I want consistent startup and session replacement, so that all entry points use my selected context.

### Control a run
6. **US06.** As a user, I want streamed message and tool progress, so that ongoing work is visible.
7. **US07.** As a user, I want ordered steering and follow-up queues, so that redirection differs from later work.
8. **US08.** As a user, I want a single abort and chat queue-text restoration, so that stopping preserves recoverable input.
9. **US09.** As an embedder, I want busy and low-level idle behavior defined, so that I do not overlap direct runs.
10. **US10.** As an extension author, I want ordered tool hooks and predictable scheduling, so that interception remains outside the loop policy.

### Use native tools
11. **US11.** As an agent, I want paged text and image reads, so that inspection reports omitted content.
12. **US12.** As an agent, I want whole-file writes and validated multi-edits, so that an invalid edit batch writes nothing.
13. **US13.** As an agent, I want native search and directory listing, so that discovery needs no downloaded search program.
14. **US14.** As a user, I want cancellable bash with partial-output evidence, so that failures remain understandable.
15. **US15.** As an embedder, I want replaceable file, search, image and process operations, so that the same tools work with controlled adapters.

### Preserve conversation history
16. **US16.** As a user, I want saved or explicitly ephemeral sessions, so that resumability is clear.
17. **US17.** As a user, I want tree navigation, fork and clone, so that exploring a branch does not erase another.
18. **US18.** As a user, I want session names and entry labels, so that important work is findable.
19. **US19.** As an agent, I want a branch-aware context projection, so that unrelated history stays out of my request.
20. **US20.** As an extension author, I want custom entries and branch reads, so that state survives resume without private database access.

### Continue long conversations
21. **US21.** As a user, I want automatic and manual compaction, so that long conversations remain usable without deleting history.
22. **US22.** As an extension author, I want before/after compaction and tree hooks, so that I can preserve relevant state.
23. **US23.** As a user, I want visible retries and guarded overflow recovery, so that recovery does not repeat completed tools.
24. **US24.** As an adapter author, I want shared store conformance, so that replacing storage preserves session behavior.
25. **US25.** As a client author, I want local notifications distinguished from execution events, so that listening implies neither persistence nor replay.

### Configure the runtime
26. **US26.** As a user, I want configured model selection and recorded changes, so that resume preserves my choices without changing startup preferences.
27. **US27.** As a team lead, I want settings layers, origins and value locks, so that local overrides cannot defeat declared requirements.
28. **US28.** As a user, I want malformed-file preservation and serialized edits, so that configuration changes do not silently erase other values.
29. **US29.** As a user, I want login, logout and serialized credential refresh, so that requests use the intended account without leaking secrets.
30. **US30.** As an embedder, I want explicit directories and offline startup, so that a library does not depend on my interactive installation.

### Extend through small interfaces
31. **US31.** As an extension author, I want declared tools, commands, flags, hooks and providers, so that adding behavior needs no caller changes.
32. **US32.** As a team lead, I want admitted host operations checked, so that a declaration cannot enlarge its engine-mediated authority.
33. **US33.** As a user, I want process failures, cancellation and idle-time reload handled, so that one extension does not corrupt other registrations.
34. **US34.** As an extension author, I want shared text interactions, so that chat, RPC and embedding need no extension-specific policy.
35. **US35.** As a package author, I want generic lifecycle, activation and branch-state hooks, so that optional capabilities need no specialized core registry.

### Install content deliberately
36. **US36.** As a user, I want registry, repository and local packages, so that shared distribution and local development use one lifecycle.
37. **US37.** As a team lead, I want pins and explicit update targets, so that an update cannot advance frozen sources.
38. **US38.** As a team lead, I want manifest-defined defaults, packages and declarations, so that governance remains data rather than loop policy.
39. **US39.** As a user, I want package-owned initialization, so that `maestro init` does not embed workflow content in the engine.
40. **US40.** As an agent, I want lazy skills, prompt templates and visible resource precedence, so that reusable instructions load predictably.

### Verify the foundation
41. **US41.** As an agent, I want a release-matched documentation pointer even with a custom prompt, so that offline contracts remain discoverable.
42. **US42.** As a maintainer, I want crate names and dependency direction checked, so that dedicated capabilities cannot enter the core by accident.
43. **US43.** As an adapter author, I want deterministic interface tests, so that conformance needs neither personal credentials nor paid services.
44. **US44.** As a user, I want Linux qualified first and other desktop platforms qualified before release, so that platform claims match evidence.
45. **US45.** As a maintainer, I want public documentation and ticket-level Docs acceptance, so that interface changes remain usable by people and coding agents.

## Implementation Decisions

### D01 — Deep modules and one composition root

A module hides behavior behind an interface. A seam permits an adapter change
without caller changes. Keep the application interface as the highest behavioral
test seam; model, storage and native-operation adapters provide controlled lower
seams. Do not create one trait or crate per source file.

This is the scoped dependency allowlist, not a request for empty crates. There
are 14 named crates in this scope, including the existing model module, executable
root and verification crate. An allowed dependency is optional, never mandatory.

| Crate | Class | One job | Allowed direct internal dependencies |
| --- | --- | --- | --- |
| `maestro-models` | CORE leaf | Invoke registered model operations. | None |
| `maestro-storage` | CORE leaf | Store session records. | None |
| `maestro-packages` | CORE leaf | Manage installed package sources. | None |
| `maestro-agent` | CORE domain | Execute the agent loop. | `maestro-models` |
| `maestro-credentials` | CORE domain | Persist provider credentials. | `maestro-models` |
| `maestro-tools` | CORE domain | Perform native tool operations. | `maestro-agent`, `maestro-models` |
| `maestro-session` | CORE domain | Maintain a conversation. | `maestro-agent`, `maestro-models`, `maestro-storage` |
| `maestro-settings` | CORE domain | Resolve effective settings. | `maestro-models`, `maestro-agent`, `maestro-packages` |
| `maestro-resources` | CORE domain | Assemble instruction resources. | `maestro-models`, `maestro-settings`, `maestro-packages` |
| `maestro-extensions` | CORE domain | Host declared registrations. | `maestro-models`, `maestro-agent`, `maestro-session`, `maestro-storage` |
| `maestro-app` | CORE application | Coordinate session operations. | All ten CORE leaf/domain crates above |
| `maestro-cli` | CORE frontend library | Translate command-line interactions. | `maestro-app` |
| `maestro` | Composition root | Wire the executable. | All twelve CORE crates above |
| `maestro-test-conventions` | Verification leaf | Check workspace conventions. | None |

The notification bus is a module inside `maestro-session`, not another crate.
`maestro-app` is the SDK: add no forwarding SDK crate. Frontends translate;
application policy stays in the application. The executable only constructs,
injects and starts. It may wire separately specified adapters later; core crates
never import those adapters, jobs or MCP. No frontend imports another frontend.

Conventions tests read Cargo metadata, declared dependencies and resolved edges.
Reject unknown members, invalid names, aliases hiding forbidden edges, cycles,
leaf imports and core-to-dedicated edges, including optional, target and build
edges. The test graph is separately acyclic; any future internal dev dependency
must target a declared dependency-free test-support crate. This map requires
none. Do not impose a crate line limit.

Deleting a domain module must move its invariants into multiple callers, not
merely remove forwarding. Native/scripted model access, memory/persistent storage,
local/controlled tool operations, file/memory settings and built-in/program
extensions justify their seams. Pure tree and projection calculations stay
concrete. Removing a dedicated package must leave the core runnable with injected
adapters and explicit ephemeral storage.

### D02 — Consume the model contract; coordinate at the application

Use the companion model specification for selection, messages, thinking, usage,
credentials, local catalogs, registration and provider instrumentation. Do not
copy its types or provider defaults into this specification. The application
assembles current system instructions, branch messages and active tool
declarations for each request; these are not system/tool-change transcript entries.

The application applies settings locks to explicit model and thinking changes,
records them in the session, and persists ordinary explicit preference changes.
Restoration changes runtime state without persisting startup preferences. Local
registration changes follow the model specification's application policy; an
already-dispatched request is never rewritten. Initial provider registration
precedes startup selection.

Application retry is distinct from a provider's setup retries. Retry eligible
transient model failures visibly; cancellation, authentication failure, exhausted
billing/quota and context overflow are not ordinary transient retries. Keep failed
attempts in raw history but omit them from the retry projection. Never rerun a
completed tool to retry its following model response. Emit retry start/end
outcomes, make the delay cancellable, and let SDK prompt completion await its
associated retry work. Do not introduce remote catalog refresh or cache warming.

### D03 — Agent lifecycle and tool execution

`maestro-agent` exposes prompt, continue, steer, follow-up, queue inspection and
clearing, abort, idle waiting and subscription. One direct run is active;
overlapping prompt/continue fails busy. Continue needs usable history. An assistant
tail requires queued input; consume steering before follow-up when restarting it.
Apply context transforms before conversion to model messages. Application-only
messages are converted or filtered, not forced into the model message union.

Both queues are FIFO with independent `one-at-a-time` or `all` modes. Poll
steering at entry and after a complete assistant/tool turn. Do not skip the
current tool batch to deliver steering. Follow-up is consumed when ordinary tool
continuation and steering are exhausted. A boolean stop-after-turn hook runs after
`turn_end`; true ends the run before polling either queue. It does not request an
invented continuation. Error or aborted assistant outcomes end the low-level run.

**Abort is one operation, not a new stop protocol.** It signals the current run;
the application also cancels its pending retry and awaits low-level idle. It does
not clear queues. The SDK separately exposes queue clearing returning steering
and follow-up text arrays. Chat restores that text, including input queued during
compaction, before aborting. RPC uses only `abort`: no queue-clear command, queue
return or implicit queue clearing. Restoration returns text, not attachments,
and is not a fence against concurrent submissions.

Model tool declarations come from the model contract. Executable tools add a
label, preparation callback, execution callback and optional execution-mode
override. Invocation carries call ID, arguments, cancellation and progress.
Results carry text/image content, arbitrary details and optional `terminate`;
execution failure is separately represented as `isError` in finalized outcomes.
There is no core output schema, structured-result field, tool-owned usage,
recovery declaration or nested-call record.

Resolve the tool, prepare current-format arguments, apply the model contract's
schema validation/coercion, then run before-call hooks in registration order.
Later hooks see earlier input replacements. **Do not revalidate after hooks.**
A block, preparation failure, validation failure or before-hook failure yields an
error without executing or running after-hooks. After-hooks apply to executed
calls, including errors; supplied content/details/error/termination fields replace
whole fields rather than deep-merging. Extension result-handler errors are
reported while preserving accumulated output; an uncaught low-level after-hook
failure becomes an error result.

Parallel batches preflight serially in assistant source order, then launch allowed
calls together. Execution-end events follow completion order; result messages
follow source order. Global sequential mode or any sequential tool serializes the
whole batch. Automatic tool continuation stops only when every finalized result
sets `terminate`; queued input may still continue unless stop-after-turn ends it.
A successful length-limited response is not by itself a reason to refuse its
complete tool calls. Partial JSON is never executable.

Cancellation is cooperative: pass it through providers, hooks and tools; await
launched work and accepted progress before finalization. Neither abort nor an
error proves external effects were undone. Await low-level subscribers in order,
including `agent_end` subscribers, before idle. Add no universal turn count,
tool-call count, concurrency cap or execution deadline.

### D04 — Native tools behind operation adapters

Tools receive a working directory and replaceable file, search, image and process
operations. Resolve relative paths there; support absolute paths and home
expansion. Model-issued and explicitly dispatched application tools use the same
validation and interception semantics. Direct user shell execution remains a
separate application operation with its own interception hook, not a fabricated
model tool call.

| Tool | Observable contract |
| --- | --- |
| `read` | Path, optional 1-based offset and line limit. Apply the window then head truncation; report beyond-file offsets, oversized first lines and continuation offsets. Sniff image bytes, support JPEG/PNG/GIF/WebP/BMP, convert BMP, fit images using the fixed profile and report dimensions or failures. |
| `write` | Path and UTF-8 content. Create parents and create or replace the whole file. |
| `edit` | Path and a nonempty array of old/new text pairs. Match every pair against the original snapshot. Reject empty old text, missing, ambiguous or overlapping matches and no-change batches before writing. Try exact matching first; if fuzzy matching is needed, use the defined whitespace/Unicode/quote/dash-normalized whole snapshot. Retain BOM and line-ending style. Return a display diff and first changed line, not a newly required patch format. |
| `bash` | Command and optional timeout in seconds. Resolve the configured shell, support a command prefix and injected environment, stream combined observed stdout/stderr, and distinguish spawn/directory failure, nonzero exit, absent exit status, timeout and abort. |
| `grep`, `find`, `ls` | Native ignore-aware traversal and the declared pattern/path/filter/limit options. Respect nested ignore files; include hidden non-ignored entries. No matches is success; invalid patterns and inaccessible roots are errors. Find returns relative forward-slash paths; ls includes dotfiles, sorts case-insensitively and marks directories. |

Write and edit share a process-local per-file mutation queue, including existing
symlink aliases. A cancelled waiter cannot release a write still in flight. This
is not cross-process exclusion or filesystem rollback. Preserve UTF-8 across
stream chunks and truncation. Shell cancellation cleans up owned process trees
and drains inherited pipes with the stated post-exit grace. Truncated shell
output always supplies a readable full-output artifact, including line-only
truncation. Retain partial output on failure; report actual truncation causes,
counts and partial-line state. No additional structured-output cap is introduced.

Normalize tool-result images through the application, including extension results
after hooks. Preserve an unconvertible extension image for the model projection
to handle explicitly. The read-only tool preset is selection, not containment.
PowerShell is deferred to the Windows phase; it is not a core tool in this scope.

### D05 — Session tree, projection and compaction

A session header records current format, identity, creation time, working directory
and optional parent locator. Entries have an identity, parent, timestamp and one
of these payloads: message, model change, thinking change, compaction, branch
summary, custom data, custom message, label or session information. Custom kinds
are namespaced data, not a closed list of extension identities. Do not add usage
entries, context-edit entries or system/tool checkpoints.

Append under the selected leaf. Navigation never deletes descendants. Latest
session information supplies the normalized name; empty domain-level names clear
it. Latest labels for valid target entries win; an unknown label target fails
without mutation. Read operations return detached views. SDK tree/entry reads do
not imply corresponding RPC commands.

Navigation to a user/custom-message entry selects its parent and returns editable
text; other targets select themselves. Current-leaf navigation is a no-op. Refuse
conflicting response, compaction or navigation work. Before-tree hooks can cancel
or customize a move; optional summarization covers the abandoned path back to the
common ancestor. Failed or aborted summarization leaves position unchanged.
Fork-before-user returns its text; at-entry fork supports cloning the active
branch. Clone refuses an empty session. Create a new session identity with only
the selected path and applicable labels/references, not queues or running work.

Before-switch/fork hooks may veto replacement. Once proceeding, settle owned
foreground work, emit shutdown, invalidate outgoing handles, bind the replacement
and emit session start. Recreate working-directory dependencies and rebind
subscriptions. Failed replacement is reported, not represented as a usable old
runtime. Inspection alone starts no model or tool work.

Project only the selected root-to-leaf path. Resolve its model/thinking changes
and latest applicable compaction: summary, retained messages, then subsequent
messages, without duplicating older summaries. Custom data and labels stay out
of model input; custom messages and branch summaries enter independently of their
display flags. Keep raw entries unchanged for inspection, statistics and export.
Resource assembly, context hooks and model conversion consume this one projection.

Automatic compaction checks the last relevant assistant outcome after a run and
before a new prompt. Trigger strictly above context window minus reserve, not at
equality. Ignore stale pre-compaction usage and overflow from another model.
Threshold compaction does not create unsolicited model work, but queued input
must still be delivered after it. Manual compaction aborts foreground work,
accepts instructions and does not itself resume the conversation.

Retain recent history without separating assistant tool calls from their results;
summarize an oversized turn prefix separately. Include the previous summary on
repeated compaction and retain cumulative file-operation information. A compaction
entry contains summary, first retained entry ID, pre-compaction token estimate,
optional extension details and whether a hook supplied it. Empty/already-compacted
input reports unavailable compaction. There are no per-model compaction overrides,
extra summary-cache policy or special length-result recovery rules here.

Await `session_before_compact` with preparation, branch entries, instructions and
cancellation; it can cancel or supply compaction content. Append the result,
rebuild context, then await `session_compact`. Cancellation before publication
appends no summary; a post-publication hook cannot undo saved history. Emit
compaction start/end with manual/threshold/overflow reason and aborted/retry/error
outcomes. Guard overflow recovery to one compact-and-retry attempt; keep the failed
assistant in raw history, omit it from retry context and never replay tool effects.

### D06 — Storage seam, not a durable execution framework

`maestro-storage` owns a session-scoped record interface, an in-memory adapter and
shared conformance cases. It contains no model or application imports. Session
entry interpretation and tree invariants belong to `maestro-session`; storage
carries their current-format records without interpreting extension data.

The interface supports creating/opening a session, reading its header and ordered
records, looking up records, appending a batch with its selected-position update,
changing the selected position, listing session metadata and closing. Session
identity scopes all reads and mutations. Mutations serialize; a successful batch
is visible as a whole, and a definite rejection leaves the prior state intact.
Readers receive detached snapshots. Distinguish definite rejection from an
uncertain write outcome; do not continue mutating an uncertain handle as though
nothing happened. Close settles admitted writes and rejects new work. No SQL
handle crosses this interface.

Base composition uses SQLite as the single authoritative persistent session
store, also serving the base's logs, audit trail, resume and replay needs. The
SQLite specification owns database layout, durable audit details, execution
ownership, crash recovery, pragmas and contention policy. This specification sets
none of those implementation defaults. The core's replay means reconstructing
history/context, not replaying tool effects or delivering a durable event bus.

Explicit ephemeral sessions use memory and report non-resumable status. There is
no alternate JSON-lines file store, dual authoritative transcript, automatic
session expiry, extension-document database or job checkpoint framework here.
Storage replacement must not change the application or frontend caller. Persistent
locators are supplied by the adapter; the existing RPC `sessionFile` and
`sessionPath` fields carry that locator, not a mandated transcript-file encoding.
Memory sessions omit it. Current-format export is presentation, not another store.

Every persistent adapter must run the same record ordering, atomic rejection,
identity isolation, snapshot, selected-position, fork and projection tests as
memory, plus its own reopen/crash tests. Persistent default operation becomes
available with the separately specified base adapter; until then the core can be
qualified through injected stores and explicit ephemeral operation, not a silent
fallback advertised as persistence.

### D07 — Three event responsibilities

The session-owned notification module supplies named-channel emit/listen,
unsubscribe and teardown. Emission does not await asynchronous observers. Catch
and diagnose handler failures without disabling other listeners. Unsubscribe
prevents future calls, not completion of already-running work. Save nothing;
late subscribers see only future notifications.

Low-level loop subscribers are different: they are ordered, awaited execution
sinks and participate in idle. Ordinary application/session observers are invoked
synchronously without awaiting asynchronous work they start. Decision-bearing
extension hooks are explicitly awaited according to their own contracts. Machine
output must drain or report failure, never silently drop events, but draining a
frontend does not turn every observer into an execution prerequisite.

Client events retain agent/turn/message/tool start-update-end, `queue_update`,
`session_info_changed`, `thinking_level_changed`, `compaction_start/end` and
`auto_retry_start/end`. `turn_end` carries the assistant and source-ordered tool
results. `agent_end` ends a low-level run, not a new universal application-settled
state. Tool events carry their call identity, name, arguments and progress or
final outcome; no parent-call/nested-call extension is added. Direct RPC shell
progress uses `bash_execution_update`; host diagnostics use `extension_error`.

JSON/RPC retains the same cumulative outer message and nested partial model event
as the model/agent contracts. Completed message and terminal outcomes remain
authoritative. Do not add delta-only serialization, append/projection events or
`agent_settled`. Session retry and compaction ordering must be tested without
claiming a settlement event that is not provided. Extension channels never create
new public RPC event families. Separate programs use admitted host requests and
hooks, not an arbitrary cross-process bus bridge.

### D08 — SDK, modes and the 29-command RPC interface

`maestro-app` creates the runtime with injected working/configuration directories,
models, settings, credentials, tools, resources, storage and interactions. Return
registration diagnostics and restoration warnings. Subscribe before prompting;
prompt completion includes the associated retry wait, not arbitrary asynchronous
observer work or an invented global settlement barrier. Disposal cancels and
releases owned work. Embedding owns neither stdout nor process exit and changes
no ambient process identity markers.

`maestro-cli` selects RPC or JSON when explicit; otherwise print when requested or
stdin is not a terminal; otherwise plain chat. Redirected stdout alone does not
change that choice. `--mode text` alone is not one-shot. RPC owns stdin. Outside
RPC, piped input, file attachments and the first positional prompt combine;
remaining prompts execute sequentially. Chat supports steering, follow-up,
queue-text restoration, text selectors, session/model/settings/login/logout/reload/
export commands and `!`/`!!` shell output included/excluded from later context.
Full-screen rendering and theme machinery are not required.

Print emits final assistant text. JSON emits a session header followed by live
events and exits; RPC emits events without that initial header and remains ready
for requests. Diagnostics go to stderr. Successful commands/help/export exit 0;
invalid invocation, startup/command/output failure and print's final assistant
error/abort exit 1. JSON may exit 0 after a represented model error: consumers must
inspect terminal events. Termination/hangup uses 143/129 where supported and
cleans up owned work. RPC EOF disposes its runtime, not background execution.

| Option group | Inputs |
| --- | --- |
| Modes | `--mode text/json/rpc`, `-p/--print`, `--export`, `--offline`, `--verbose`, help/version |
| Models | `--provider`, `--model`, `--api-key`, `--thinking`, `--models`, `--list-models [search]` |
| Sessions | `-c/--continue`, `-r/--resume`, `--session <locator-or-id>`, `--fork <locator-or-id>`, `--session-dir`, `--no-session` |
| Tools | `-t/--tools`, `-nbt/--no-builtin-tools`, `-nt/--no-tools` |
| Resources | Repeatable extension, skill, prompt-template and append-system-prompt inputs; system-prompt replacement; discovery-disable inputs for extensions, skills, templates and context |

A provider-only selection uses the model specification's fallback policy; it does
not newly require a model argument. Unknown short options fail; long extension
flags must be declared. Explicit tool allowlists apply to built-in, extension and
custom tools; disabling built-ins leaves independent registrations eligible.
Explicit resources can load when automatic discovery is disabled, subject to
locks. RPC refuses positional file attachments. Do not add session-ID/name,
exclude-tools or project-trust flags from a larger command surface. SDK custom
session IDs remain supported independently of CLI options.

RPC framing is UTF-8 JSON objects delimited by LF, accepting CRLF and a final
unterminated record; Unicode line separators are not record separators. Commands
have `type` and optional string `id`. Responses use `type: response`, `command`,
`success`, optional `data` on success or a string `error` on failure. Recognized
requests retain their ID; parse failures use command `parse` without an ID, and
unknown-command failures have no correlation guarantee. Invalid requests do not
terminate the process. Requests can overlap and replies can arrive out of order.

Prompt success is emitted once after successful preflight, also for queued or
immediately handled input. It has no disposition data. Preflight failure is that
request's failure reply; a later model failure is an event, not a second reply.
Busy prompts must specify steer/follow-up behavior. Direct `steer`/`follow_up`
reject extension commands instead of running command handlers.

| # | Command | Arguments beyond type/id | Success data |
| --- | --- | --- | --- |
| 01 | `prompt` | message, images?, streamingBehavior?: steer/followUp | Absent |
| 02 | `steer` | message, images? | Absent |
| 03 | `follow_up` | message, images? | Absent |
| 04 | `abort` | None | Absent, after low-level idle |
| 05 | `new_session` | parentSession? | cancelled |
| 06 | `get_state` | None | Session state |
| 07 | `set_model` | provider, modelId | Model |
| 08 | `cycle_model` | None | model, thinkingLevel, isScoped; or null |
| 09 | `get_available_models` | None | models |
| 10 | `set_thinking_level` | level | Absent |
| 11 | `cycle_thinking_level` | None | level; or null |
| 12 | `set_steering_mode` | mode: all/one-at-a-time | Absent |
| 13 | `set_follow_up_mode` | mode: all/one-at-a-time | Absent |
| 14 | `compact` | customInstructions? | Compaction result |
| 15 | `set_auto_compaction` | enabled | Absent |
| 16 | `set_auto_retry` | enabled | Absent |
| 17 | `abort_retry` | None | Absent |
| 18 | `bash` | command | Shell result |
| 19 | `abort_bash` | None | Absent |
| 20 | `get_session_stats` | None | Session statistics |
| 21 | `export_html` | outputPath? | path |
| 22 | `switch_session` | sessionPath | cancelled |
| 23 | `fork` | entryId | text when supplied, cancelled |
| 24 | `clone` | None | cancelled |
| 25 | `get_fork_messages` | None | messages: entryId/text pairs |
| 26 | `get_last_assistant_text` | None | text, or null text |
| 27 | `set_session_name` | name | Absent |
| 28 | `get_messages` | None | messages |
| 29 | `get_commands` | None | commands: name/description?/source/sourceInfo records |

Named fields in the last column are fields of a data object; Model, Session state,
Session statistics, Compaction result and Shell result are the data object itself.
Cycle commands use null data when unavailable; last-assistant-text instead uses
an object whose text can be null. State contains optional model, thinking level,
streaming/compacting flags, queue modes, session identity/name/locator, automatic
compaction flag and message/pending counts. Statistics expose message/tool counts
and reported usage/cost; stale context estimates after compaction stay unavailable.
Vetoed replacement succeeds with `cancelled: true`. RPC rejects empty trimmed
session names even though the domain permits clearing them.

`get_commands` lists extensions, templates and skills, not chat-only built-ins.
The RPC shell result has output, optional exit code, cancelled/truncated flags and
optional full-output locator. A nonzero shell exit is a completed shell result,
not a failed RPC dispatch. `abort_retry` and `abort_bash` remain their existing
specialized controls, not additional names for stopping an agent run. UI response
records are separate from the 29-command union. Export escapes untrusted content,
retains raw-history integrity and needs no browser runtime inside the engine.

### D09 — Settings, credentials and process inputs

`maestro-settings` resolves engine → manifest → user → project, carrying value
origins and locks. Objects merge recursively; ordinary arrays/scalars replace.
Resource discovery has its own additive rules. Locks name property segments and
freeze values or subtrees; arrays are atomic. Replacing an ancestor cannot bypass
a descendant lock. Equal restatement is allowed. Conflicts, missing lock targets
and invalid manifest locks fail with origins, not secret values. CLI, environment,
runtime and extension setters must use the same lock enforcement.

File edits serialize read-modify-write, preserve unrelated edits and owned unknown
keys, and never overwrite a malformed file. Reload preserves the last usable
snapshot and reports errors; an in-memory settings adapter retains its state on
reload. Resource reload waits until between runs. Settings-value locks are not
file-write locks. Native contention mechanisms must preserve serialized updates
without copying a busy-spin implementation or silently choosing new lease limits.

Working directory and configuration root are explicit SDK/process inputs. Session
location precedence is explicit option, branded environment input, effective
setting, then the configured user root; applicable locks still win. Resource
paths resolve relative to their declaring scope; invocation paths and relative
session-location settings use the working directory. Provider environment names
are registration data, not hard-coded selections. Offline suppresses automatic
startup network work, not explicit model requests or arbitrary program access.
No model-callable-shell session metadata injection is required.

There is no general project-trust database or trust prepass. Loading project
instructions or declared executable extensions can have effects under ordinary
user permissions. Value locks are not a sandbox and do not make project content
safe. Dedicated packages may later own their specific admission rules without
introducing a generic trust subsystem here.

`maestro-credentials` implements the model contract's credential-storage seam
using private provider files plus memory/read-only adapters. Store API-key or
provider-owned refreshable-token data; do not expose credentials through ordinary
listing, logs, events or transcripts. Metadata inspection does not run secret
helpers. Serialize refresh, re-read under the lock, preserve valid state on failed
reads/refreshes and save rotated state before release. Request authentication
precedence and provider token exchange belong to the models contract, not another
resolver here.

For configured secret values, a leading `!` explicitly requests a lazy shell
helper; otherwise resolve a nonempty environment variable of that name, then use
the literal. Cache helper results, including unresolved failures, for the process
lifetime or until explicit reset. Do not add interpolation/escaping grammar.
Login/logout use dedicated interactions, not conversation messages. Cancellation
reaches prompts, requests and storage waits; logout removes local credentials,
not remote grants or ambient variables. Private files are not an encrypted vault.
Do not add auth-check or credential-export command families.

### D10 — Declared extensions and shared interactions

`maestro-extensions` admits native programs or composition-wired built-ins through
one interface. A declaration fixes host-assigned owner identity, entry/factory,
allowed registrations, host operations and state scope. Local development still
requires a declaration. Validate mediated calls against that grant. This is
engine-mediated authority, not operating-system containment; built-ins have no
process-crash isolation. Programs are not loaded as native libraries or embedded
language modules.

A program receives `hello` with host-instance/session/grant context, replies with
its complete `register` batch, and receives `ready` only after atomic admission.
Failure publishes no registrations. Use one current JSONL grammar with correlated
requests, operation/target/context/input, result or coded error, progress,
notifications and targeted cancellation; stderr is diagnostic. Replies may be
out of order. Reject stale-instance calls; late replies cannot revive cancelled
work. No version negotiation, universal frame cap or implicit hook timeout is
introduced. Built-ins skip transport, not admission semantics.

Tools, commands with argument completion, boolean/string flags, provider adapters
and lifecycle hooks are generic registrations. Dynamic tools refresh after binding;
all-tool inspection and active-tool get/set remain separate from registration.
First tool wins across extensions with diagnostics; a registration may replace
its owner's tool, and an admitted extension tool may replace a built-in. Duplicate
commands receive deterministic numbered invocation suffixes. Provider replacement
follows the model contract. Reserved engine administration names cannot be
shadowed. Process reload waits for idle; runtime registration updates do not imply
process reload.

Hook dispatch snapshots load/registration order. Keep resource discovery, session
start/shutdown, before switch/fork/tree/compact, after tree/compact, input,
before-agent-start, context, model/thinking selection, message/turn/run/tool events,
tool call/result and direct-user-shell interception. Message-end replacement
preserves the role. Input transforms chain; handled input short-circuits. Payload
transforms and response observations use the model contract's existing provider
hooks, not header/raw-stream additions. A failed direct-shell interceptor cannot
silently fall through to execution. Awaited control operations are command-context
operations; prevent callback-induced waits on their own run.

A failed program or malformed protocol settles its outstanding calls as errors,
invalidates owned handles and leaves other extensions usable. Never replay its
effects. An unavailable blocking hook does not become permission to proceed.
Reload tears down subscriptions and dialogs before replacing registrations;
failed replacement stays unavailable, without automatic rollback.

Generic hooks sufficient for later packages are part of this core: scoped working
directory/model/cancellation context; available-model and authorized provider-auth
access; injected model completion; registered tools and active-set control;
commands/flags; resource discovery; custom/user message submission; custom-entry
append and branch reads; session lifecycle; dialogs; explicit process execution;
and in-process notifications. Setting/artifact writes use the owning serialized
operation, not a tools-crate import. Separate-program decisions use declared
request/reply operations, not serialized callbacks or an awaited notification bus.
Package-specific catalogs, consent, protocol connections and state schemas remain
with those packages. Add no core MCP registry, tool namespace/deferred-search
metadata, programmatic nested execution or workflow scheduler.

Every UI request uses `type: extension_ui_request`, a unique ID and one of the
following nine methods. Dialog responses use `type: extension_ui_response`, its ID
and exactly one of `value`, `confirmed`, or `cancelled: true`. They receive no RPC
command response.

| Method | Fields | Completion |
| --- | --- | --- |
| `select` | title, string options, timeout? | value or cancelled |
| `confirm` | title, message, timeout? | confirmed or cancelled |
| `input` | title, placeholder?, timeout? | value or cancelled |
| `editor` | title, prefill? | value or cancelled |
| `notify` | message, notifyType? | No response |
| `setStatus` | statusKey, statusText? | No response; omitted text clears |
| `setWidget` | widgetKey, widgetLines?, widgetPlacement? | No response; omitted lines clear |
| `setTitle` | title | No response |
| `set_editor_text` | text | No response |

Chat uses text interactions, RPC relays them, and SDK callers inject an adapter.
Print/JSON have no UI: confirmations return false, other dialogs return no value,
and presentation notifications do nothing. Cancellation/timeout resolves the same
way; ignore unknown, duplicate and expired responses. Only select/confirm/input
accept caller timeouts; editor has none. Teardown resolves outstanding requests.
Capability checks must distinguish text dialogs from unavailable full-screen
widgets/overlays. Do not add a separate structured-question protocol.

### D11 — Packages, manifest, resources and documentation

`maestro-packages` handles `npm:` registry archives, repository sources and local
paths as native package sources. Use Git through its command; acquire and extract
registry archives natively. Run no package-manager program, lifecycle script,
dependency installation or embedded language runtime. Packages contain metadata,
resources and declared native programs. A missing platform executable is
unavailable, not permission to build it. Extraction cannot escape its destination;
local content stays in place. Additional extraction/image/rendering libraries
still need approval before adoption.

Install/remove/list/config use personal scope unless local is selected. Uninstall
aliases removal; removing a local source does not delete its working directory.
Identity ignores registry version/repository ref and normalizes local location.
Project declarations take precedence over personal declarations for the same
identity. Filters narrow declared resources: omitted means all, empty means none;
exclusions and exact selectors preserve ownership. Autoload filtering must not
silently expand a package's declaration. Offline startup never acquires missing
content; online resolution can acquire declared missing content.

Bare `maestro update` updates the engine and packages. `--self`, `self` or
`maestro` selects only the engine; `--extensions` selects installed packages;
`maestro update <source>` selects one configured package identity. Keep explicit
single-extension selection and `--force` reinstall behavior. There is no update
`--models` or `--all` switch. Exact source versions and full commits never advance,
even when forced: teams can freeze a prior package version. A source target is
not permission to change its ref. Missing pinned installations can be repaired;
local sources do not update. Preserve distinct per-source completed/skipped/failed
outcomes, rather than claiming whole-set atomicity. Startup checks advise only;
updates become active at the next permitted reload/start. Respect externally
managed executable installation. Add no engine-compatibility pin or rollback layer.

The governed manifest is data consumed by these existing modules, not a workflow
interpreter. Its common envelope carries identity/provenance, package sources and
pins, resource selections, settings/locks and extension declarations. Domain
resource kinds and schemas are registered by their owning packages with defaults;
never close agent roles, workflows or vocabularies into core enums. Governance and
workflow content stay outside the loop. `maestro init` dispatches to its admitted
package command and reports unavailable when that package is missing; the engine
does not own templates, language detection or repository policy.

`maestro-resources` supplies ordered catalogs with provenance and diagnostics.
Explicit resources precede project-local then personal resources; local resources
precede package resources within their discovery rules. Deduplicate real files;
diagnose missing, malformed and colliding entries. Resource hooks can contribute
skill/template locations at startup/reload without taking over global discovery.

Skills advertise name, description and location; load bodies on demand. An
explicit-only skill is not automatically advertised. Stop recursive discovery at
a skill root. Reject missing descriptions or malformed declarations; name/length
warnings alone do not reject a skill. First discovery wins. Explicit invocation
remains possible without advertised skill commands. Templates use filename names,
front-matter/body descriptions and quote-aware positional/all/default/sliced
arguments with one-pass expansion, not shell evaluation. Extension commands run
before template/skill expansion; ordinary input hooks see raw input first.
Unknown template input is unchanged.

Context instructions load personal then ancestor-to-working-directory. Use
Maestro's context-file names, not foreign-tool discovery aliases. Project system
replacement/addendum takes precedence over its personal counterpart. Assemble
instructions, current-tool guidance, addendum, context, visible skills and working
directory. Extension transforms can change the prompt, but an engine-owned
release-documentation pointer must survive the final result, including custom
prompts. No named-section transcript or nested-worktree override machinery is
needed.

Ship release identity, a task index, concept references, glossary, current schemas,
release notes and examples, usable offline from a locator independent of working
directory. Generate factual tool/command/setting/event inventories from owning
definitions; do not create another runtime registry for documentation. Verify
installed links, fragments, assets, schemas and deterministic examples. Explain
permissions, cancellation, pins, defaults and adapter seams alongside interfaces.

## Defaults and Limits

Each owner defines its values once; tests cover omitted, explicit, disabled and
boundary cases. These are operation-local defaults, not new whole-run budgets.
The models specification owns provider/model values; personal settings are not
product defaults. No SQLite, jobs or MCP implementation defaults are selected here.

| ID | Owner / area | Value and meaning |
| --- | --- | --- |
| C01 | Agent queues/scheduling | Both queues `one-at-a-time`; batches parallel unless globally or per-tool sequential. No universal turn/call/concurrency/deadline cap. |
| C02 | Tool selection | Default read/bash/edit/write. Read-only preset read/grep/find/ls. Selection is not containment. |
| C03 | Read/shell output | Head for read, tail for shell: first of 2,000 lines or 51,200 bytes. Notices are outside the payload cap. Read offset starts at 1. |
| C04 | Search/list output | Grep 100 matches, find 1,000 results, ls 500 entries; 51,200-byte payload cap; grep line prefix 500 characters. No extra 2,000-line search/list cap. Grep is case-sensitive regex with zero context unless specified. |
| C05 | Shell | No default timeout; a positive supplied seconds value enables it, nonpositive leaves it disabled. Prefix absent. Progress throttle 100 ms; inherited-pipe grace 100 ms after process exit, not reset by each chunk. No borrowed runtime timer ceiling. |
| C06 | Images | Auto-resize true, block-images false; fit within 2,000 × 2,000 pixels and below 4.5 MiB of base64 payload, JPEG quality 80. Fitting qualities 80/85/70/55/40 and dimension factor 0.75 down to 1 × 1; MIME sniff up to 4,100 bytes. |
| C07 | Edit | At least one replacement, old text nonempty, display-diff context 4 lines. No additional edit-count/file-size cap. |
| C08 | Compaction | Enabled; reserve 16,384 tokens; retain recent 20,000 tokens; strict greater-than threshold. No per-model budget overrides. |
| C09 | Summary requests | History output floor(0.8 × reserve), split-prefix floor(0.5 × reserve). Branch reserve 16,384 tokens, output request 2,048 tokens, missing-window fallback 128,000 tokens; skip-summary-prompt false, navigation summary opt-in. Normal model request limits still apply. Tool text in summaries clips at 2,000 characters. |
| C10 | Application retry | Enabled, 3 additional attempts; delay 2,000 × 2^(attempt − 1) ms, attempt starts at 1, no jitter or application-delay cap. Separate from provider retry. |
| C11 | Persistence/notifications | Base uses SQLite through the seam; explicit memory mode is non-resumable. No automatic session expiry. Local notifications are unsaved, with no subscriber replay. |
| C12 | Modes/RPC | Explicit RPC/JSON first; otherwise print if requested or stdin is not a terminal, chat otherwise. Exactly 29 commands; no added generic command deadline or frame-byte cap. |
| C13 | Settings/resources | Engine → manifest → user → project except locks. Package/extension/skill/template lists empty; skill commands enabled; quiet-startup/hide-thinking/collapsed-changelog false where applicable to plain presentation. |
| C14 | Credential files/helpers | New private file 0600 and parent 0700 where supported. Secret helper timeout 10,000 ms; process-lifetime result cache. These are not encrypted-storage guarantees. |
| C15 | File contention | Synchronous file-store acquisition: 10 attempts with 20 ms spacing. Asynchronous credential policy: 10 retries, factor 2, randomized 100–10,000 ms backoff, stale-lock age 30,000 ms. Native mechanism/library selection remains gated; do not substitute a newer 30-second overall wait or busy spin. |
| C16 | Extension programs | No invented numeric handshake, hook, frame or shutdown deadline. Cancellation and ownership are required; finite termination of arbitrary programs is not promised. |
| C17 | UI | Nine methods. No implicit dialog timeout; caller milliseconds only for select/confirm/input. Notify type info; widget placement aboveEditor. Missing UI/cancel means false or absent, never consent. |
| C18 | Skills/templates | Name warning at 64 characters, description warning at 1,024. Template fallback is first nonempty body line clipped to 60 characters plus ellipsis. Explicit-only false unless declared. |
| C19 | Packages | Personal scope; omitted resource filter all, empty none. Exact pins fixed; local sources unchanged. Bare update includes engine and packages. Metadata/repository update pools 4/4; metadata lookup timeout 10,000 ms. No general install deadline. |
| C20 | Locations | SDK working directory is process cwd unless supplied; configuration uses the selected user root, sessions beneath it unless overridden. Never read an unrelated installation implicitly in tests. |

## Testing Decisions

Use failing observable-behavior tests before implementation. Test through the
highest useful interface: most application behavior through `maestro-app`, lower
contract conformance at its owning seam, and frontend parsing/framing/output in
frontend suites. Substitute scripted providers, memory stores, controlled clocks,
barriers and isolated native-operation fixtures, not the implementation under test.
Replace superseded detail-coupled tests rather than maintaining two implementations.

The existing workspace supplies metadata/architecture and Rust-comment convention
fixtures; extend those instead of creating a second convention framework. There
are no implemented runtime tests to claim as prior art yet. New conformance suites
must be shared by adapters, not merely similar tests with weaker fake assertions.

| Test group | Stories | Decisions | Required observable proof |
| --- | --- | --- | --- |
| T01 — Application/frontends | US01–US05 | D01, D07, D08 | Shared SDK behavior; runtime replacement/rebinding/disposal; real subprocess stream/exit tests; all 29 commands and response shapes; preflight success/failure exactly once; fragmented UTF-8, CRLF and final records; cumulative outer/nested event snapshots. |
| T02 — Agent lifecycle | US06–US10 | D02, D03, D07 | Busy/continue cases; FIFO modes and steering priority; stop-after-turn; chat text restore versus RPC abort preserving queues; awaited loop listeners versus ordinary observers; serial preflight, completion-order events and source-order results; hooks without revalidation; all-result termination and cooperative cancellation. |
| T03 — Native tools | US11–US15 | D03, D04 | Original-snapshot edits, zero writes on invalid batches, aliases sharing mutation order; ignore/glob rules; Unicode and line/byte bounds; image fitting; shell process-tree cleanup and readable full artifacts for every truncation cause. |
| T04 — Session semantics | US16–US20 | D05, D06 | Append-only branching, names/labels, navigation veto/failure, fork/clone reference integrity, selected-path projection, custom data excluded/custom messages included, raw-history-preserving escaped export, ephemeral status and adapter-independent locators. |
| T05 — Recovery/storage/events | US21–US25 | D02, D05, D06, D07 | Compaction equality/crossing, retained tool pairs, repeated summaries and stale-usage guards; before/after hook order; queued work after compaction; one overflow recovery; retry without tool replay; memory conformance for atomic rejection/snapshots/identity/order; notification failure/unsubscribe/teardown without persistence. |
| T06 — Configuration/authentication | US26–US30 | D02, D08, D09 | Companion model integration; explicit preference persistence versus restore; every lock bypass path; malformed/concurrent-file preservation and memory reload; serialized auth refresh and secret redaction; injected locations and offline startup. |
| T07 — Extensions/interactions | US31–US35 | D03, D07, D10 | One suite for built-in/program admission, current handshake and grants, collision rules, dynamic activation, idle reload, crash/stale replies, unavailable blocking hooks, role-preserving replacements, generic package hook fixture and all nine UI methods with no-UI/cancel/timeout cases. |
| T08 — Packages/resources | US36–US40 | D09, D10, D11 | Registry/repository/local sources, contained extraction, exact-pin/force/local behavior, update target parsing and partial outcomes; manifest ownership/locks; missing initialization package; precedence, lazy skills and nonrecursive templates. |
| T09 — Structure/defaults/docs | US41–US45 | D01, D11 | Every forbidden dependency/name/alias/feature/target fixture; same callers with adapter swaps; core runs without dedicated packages; every C01–C20 row; retained final docs pointer, installed links/schemas/examples and documented public interfaces. |

The mapping covers all 45 stories and all 11 decision groups. These are test
families, not implementation tickets or effort estimates. Tickets derive from the
merged specification only; each must name its story/decision coverage, behavioral
red-first test and commands without copying this specification.

### Quality and Docs acceptance

Use Linux for checks and tests now. Restore macOS and Windows checks, including
native file/process/credential behavior and the separately specified PowerShell
work, before the first release. No mutation testing, coverage target or new
per-repository quality configuration is part of this bar.

`mise.toml` pins just, prek and jaq; `rust-toolchain.toml` pins Rust through rustup.
`just setup` installs the hooks. `just check` runs formatting, Clippy with warnings
denied, public rustdoc with `RUSTDOCFLAGS="-D warnings -D missing_docs"`, and
workspace conventions. `just test` runs workspace tests; `just ci` runs both,
matching shared Linux CI. The pre-commit hook runs `just pre-commit`: format,
re-stage only the already-staged files, then `just check`, not the full runtime
test suite. Use the workspace's capped build runner on the workstation.

Every implementation ticket must include this **Docs** acceptance block:

1. Public rustdoc documents every public item and crate root; strict docs build passes.
2. The feature's `docs/` page explains behavior, errors, configuration and examples.
3. `AGENTS.md` or `CONTEXT.md` is updated when coding agents need a new rule, command or term; otherwise record why no update is needed.
4. `CHANGELOG.md` has the change under `[Unreleased]`.
5. Both Standards and Spec reviewers verify this block, adapter substitution and module depth; the merged spec is their single scope reference.

Rust comments describe code only: no planning IDs, numbered slices/tasks, ticket
or pull-request references. Conventions enforce that distinction. CI mechanically
checks rustdoc; review checks the remaining documentation obligations. Fix slow
tests instead of accepting them as normal. No runtime conformance is claimed by
publication of this document.

## Out of Scope

1. MCP client/gateway/inbound tools, protocol-specific consent, Tasks and caches;
   restartable jobs; SQLite implementation, detailed logs/audit schema and crash
   recovery. They are separate base or later specifications, not core dependencies.
2. PowerShell until Windows qualification; full-screen/desktop clients, themes,
   application daemons and remote multi-session serving.
3. Supervisor/subagents, todo, structured-question policy, memory-backed default
   backend, knowledge, intelligence and workflow/bootstrap content. These remain
   base capabilities delivered slice by slice through replaceable packages.
4. System/tool-change replay, context edits, nested execution/records, deferred
   model responses, remote catalogs, generic project trust, cache warming,
   richer auth commands and newer extension event/hook families.
5. Durable event delivery/subscriber cursors, arbitrary effect replay, old formats,
   migrations, compatibility/refusal layers, archives/rollback, operating-system
   containment, automatic retention and exactly-once effects.

## Further Notes

This specification is a scope contract, not implementation authorization. Its
independent review and owner-approved pull request must land on `main` before
implementation tickets are created. The companion model amendment must also land;
no caller should implement the superseded model shapes while these changes are
pending. Later slices receive short specifications when reached rather than a
larger speculative plan here.

No additional library is selected. Native archive extraction, image processing,
HTML rendering and file-lock support may require approval beyond the current
foundation when their implementation is reached. They are not permission to add
runtime wrappers or new dependencies silently.

Residual limits are explicit: declarations and read-only presets do not sandbox
code; transformed tool inputs are intentionally not revalidated; abort is not
rollback or remote queue handback; stdout snapshots and unbounded JSONL input can
be large; arbitrary hooks/programs have no implicit deadline; private provider
files and full-output artifacts may contain sensitive data. Storage durability
and platform promises require their own adapter qualification, not inference from
passing memory tests.
