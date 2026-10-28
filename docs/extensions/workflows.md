# Extension author workflows

Choose the callback that owns the change you need. Start from a shipped example,
keep its factory and component export, and use the [build and load
recipe](../extensions.md#create-the-project). This page connects author tasks to
those examples; the [event and result inventory](guest.md#events-and-results)
remains the field reference.

Loading a component grants it the user's access, including files, environment
and network. Tool hooks are application policy, not a security boundary. Test
file-changing extensions in a disposable directory before loading them in a
working project.

## Follow a prompt through the host

On session initialization, `session_start` runs before `resources_discover`.
Discovery contributes skill, prompt and theme paths; it does not load another
extension. Reload uses the `reload` reason; other session starts use `startup`
for resource discovery. Rebuild state before contributing paths that depend on
it. Use the event's supplied working directory rather than the extension's build
directory. See [resource loading](../application/resources.md).

For an ordinary idle prompt, the useful intervention points are:

1. An extension slash command is checked before `input`. A matched command handles
   the submission itself. Otherwise `input` sees unexpanded text and can transform
   it or finish locally. Skill and prompt-template expansion follows.
2. `before_agent_start` receives the assembled system prompt and its structured
   inputs. It can add custom messages or replace the prompt for this run.
   `agent_start` begins the agent's work; a run can contain several turns.
3. Each turn emits `turn_start`. Before a model request, `context` can replace the
   projected messages. `before_provider_request` sees the provider payload;
   `after_provider_response` observes status and headers before body consumption,
   where the transport exposes them. Assistant streaming uses `message_update`;
   finalized messages pass through `message_end`.
4. A tool call emits `tool_execution_start`, then preparation and validation run
   before `tool_call`. Allowed calls execute, possibly emitting
   `tool_execution_update`. `tool_result` can patch a completed result before
   `tool_execution_end` and the final tool-result message.
5. `turn_end` closes a turn. More tool-driven turns can follow before `agent_end`.
   Do cleanup for a runtime's lifetime in `session_shutdown`, rather than treating
   every turn end as shutdown.

Message start/end events also cover user and tool-result messages. Do not infer
one model request from one user prompt: tools, retries and continuation can
produce more work. In parallel tool execution, siblings are preflighted in source
order, updates can interleave, and results finish in completion order. Final
result messages retain assistant source order. A tool-call handler must not rely
on seeing its siblings' completed results in session history.

Handlers run in extension load order and then registration order. A returned
replacement becomes the next handler's input on the routes that chain changes.
Notification return values are ignored. See [dispatch rules](hooks.md) before
combining several handlers; errors and cancellation do not follow one universal
policy.

## Choose an event handler

### Transform input or answer without a model

Use [`input_transform.rs`](../../examples/extensions/input_transform.rs):

1. Build and load the example using the component recipe. In an interactive host,
   submit `ping`; the handler reports `pong` and returns `Handled` without a model
   request.
2. Submit `?quick Explain the parser`. The handler rewrites the text and lets the
   host continue through expansion and normal prompting. This path uses the
   configured provider and can incur usage.
3. When adapting the handler, check `source` if extension-injected text should
   bypass your rule. Preserve attachments unless you mean to replace them.

Transforms chain. Missing or null replacement images keep the current images;
an explicit empty list removes them. `Handled` stops later input handlers and
agent prompting, so send any local feedback before returning it. See
[input transformation](guest.md#an-input-transform-hook).

### Change guidance, request context or provider data

Use [`prompt_customizer.rs`](../../examples/extensions/prompt_customizer.rs) to
add tool-aware guidance. Read `system_prompt_options` supplied with
`before_agent_start`; it already describes selected tools, skills, context files
and user additions. Extend the supplied `system_prompt` instead of rediscovering
resources or appending the user's addition twice.

For temporary message filtering, return replacement messages from `context`.
That changes the model-request projection, not the saved session tree. Use
`message_end` when the finalized message itself must change, and keep its role.
A changed-role replacement fails.

Use `before_provider_request` for provider-specific diagnostics or payload edits,
not for changing session history. No result preserves the payload; a supplied
JSON value replaces it, including null or an empty object. A payload-level prompt
edit does not change the application's system-prompt reader. Keep credentials
and private conversation content out of diagnostic logs.
`after_provider_response` is observational; header availability depends on the
transport. The [model-stream guide](guest_streams.md) covers provider callbacks.

### Restore state and handle session changes

Use [`todo.rs`](../../examples/extensions/todo.rs) for branch-local state and
[`tools.rs`](../../examples/extensions/tools.rs) for a saved tool selection.

1. Keep live state in the factory's captures. Multiple commands and tools in the
   same factory can share it; separately loaded instances do not share captures.
2. Save reconstructible snapshots in tool-result `details` or explicit custom
   session entries. Tool `content` is the model-facing answer; details also serve
   rendering and replay. An in-memory update alone will not survive reload.
3. Rebuild from the active branch at `session_start` and `session_tree`. Clear
   live state before replay so a branch with no snapshot starts empty.

The todo example uses the newest matching tool result on the current branch.
The Snake example deliberately reads its newest save from the whole session.
Choose that policy explicitly: whole-session replay can import state from a
branch the user left. See [session entries](guest.md#session-entries).

Before switching or forking, a session-before handler can cancel. Compaction and
tree navigation have their own before/after pairs; return a custom summary only
through their documented result types. Start from
[`trigger_compact.rs`](../../examples/extensions/trigger_compact.rs) and the
[compaction guide](../sessions/compaction.md#extension-integration).
After successful replacement, use the fresh context supplied to `with_session`.
Return after awaiting reload. The old callback's context does not become the new
session's context. See [context lifetimes](lifecycle.md).

For a model-dependent status or cache, listen to `model_select` and
`thinking_level_select`. Model selection can clamp the thinking level first;
update from the supplied effective values rather than assuming the requested
level survived. These notifications do not veto selection. Use
[`status_line.rs`](../../examples/extensions/status_line.rs) for themed status
updates and [selection actions](guest.md#model-selection-and-action-errors) for
changing the selection, including authentication failures.

### Inspect tools and user shell commands

Use [`protected_paths.rs`](../../examples/extensions/protected_paths.rs) to
inspect a tool's name and arguments, and
[`permission_gate.rs`](../../examples/extensions/permission_gate.rs) to ask for
approval. Both examples have deliberately limited matching rules; neither is a
filesystem or shell security policy.

`tool_call` runs after validation. Later handlers see argument edits, but the host
does not validate those edits again. Validate any replacement yourself. Return an
explicit block to refuse execution; a tool-call error also fails the call. A name
check alone does not prove a built-in argument shape because an extension can
replace that name. Decode and check the JSON you actually received.

`tool_result` patches only the fields supplied in its result; omitted content,
details and error observations keep their previous values. Use it to redact or
annotate an answer, and preserve unrelated details needed for replay or rendering.
Use execution notifications for progress displays, rather than blocking policy.

`user_bash` handles direct user shell input separately from the model's `bash`
tool. Its event includes the working directory and whether output is excluded
from context. Return operations to change execution, or a completed `BashResult`
to replace it entirely; the first supplied result wins. Start from the
[local shell component](../../crates/maestro-extensions-wasm/examples/local_shell_component.rs)
and [native shell operations](guest_operations.md#run-raw-local-shell-operations).
Forward cancellation, timeout and byte-output callbacks when wrapping operations.
Writing raw stdout can corrupt JSON or RPC output; use the host's UI or event
channels for feedback.

## Build a tool without losing host behavior

### Define the contract and settle the call

Start from [`hello.rs`](../../examples/extensions/hello.rs). Give the tool a stable
name, a readable label, a model-facing description and a parameter schema.
`prompt_snippet` adds its short entry to the default prompt's available-tool list;
`prompt_guidelines` adds instructions only while the tool is active. Name the tool
inside each guideline, since the host presents the bullets without grouping them
under tool names. These fields do not replace the model-facing description.

Use `prepare_arguments` for input normalization before schema validation. Its
in-place edits affect retained assistant arguments; its separate return value is
what gets validated. Preparation should not perform file effects. The native edit
factory already carries its preparation; preserve it when wrapping edit.

Execution receives the call ID, prepared arguments, optional cancellation signal,
optional progress observer and a fresh context. Check cancellation before starting
an effect and pass the signal to native work. A progress result is an update, not
the final answer. Return the final `AgentToolResult` after the work settles.

Return `Err` for a failed execution. Returning `Ok` with text that says "failed"
or an error-looking detail does not set the execution error flag. Cancellation
cannot undo writes or remote actions that have already completed; report that
constraint rather than claiming rollback.

A result with `terminate` present and true asks to skip the next automatic model
continuation. It takes effect only when every finalized result in the batch is
terminating. It does not kill other calls in that batch, shut down Maestro or
replace the explicit shutdown operation. Queued input can still require work.
See [tool callbacks](guest.md#tools) and [agent execution](../agent/api.md#tools-hooks-and-errors).

### Wrap a built-in instead of rewriting it

Registering the same name replaces a built-in's execution. To add auditing,
access checks or remote I/O while keeping its behavior:

1. Obtain the registerable native definition with `create_tool_definition` or a
   specific factory such as `create_read_tool_definition`. Construction itself
   does no file I/O. Use the intended execution directory.
2. Retain the original execution callback. In your wrapper, perform the added
   check, then delegate the ID, arguments, signal, update callback and context.
   Keep the definition's schema, preparation and prompt metadata.
3. Register that definition. For rendering-only changes, wrap only the desired
   renderer and forward its supplied context, theme and previous component.

The [native-definition registration example](../../crates/maestro-extensions-wasm/examples/context_bridge/builtin_registered.rs)
wraps edit execution with progress while keeping native matching and queueing.
[Native operations](guest_operations.md#wrap-and-register-a-read-definition)
explains renderer delegation: the opaque `native_row` is required by a native
renderer, and a fabricated context cannot replace it.

Execution and rendering are independent. For an override of a built-in name,
each omitted renderer slot inherits that built-in's slot. A newly written
override does not inherit the built-in's prompt snippet or guidelines. Keep the
result content and details compatible with any renderer you retain, or provide a
renderer for your new result shape.

The [`tool_override.rs`](../../examples/extensions/tool_override.rs) example
illustrates auditing but implements a simplified text reader. It loses native
image and truncation features; it is not the recipe for a behavior-preserving
read wrapper. Its log contains paths and access decisions, so treat it as private
data. See the [example's exact limits](../../examples/extensions/README.md#what-the-audited-read-override-does).

### Delegate remote work and order mutations

Replace native observations through `ReadOperations`, `WriteOperations`,
`EditOperations`, `BashOperations`, `FindOperations`, `GrepOperations` or
`LsOperations`. Keep result formatting, matching and output policy in the native
tool. Pick the interface that covers the actual remote work: replacing grep's
file observations does not move its ripgrep process to another machine.

Read callbacks receive resolved paths and return original bytes. A shell backend
receives command, directory and execution options; a bash spawn hook can edit the
command, directory and environment before execution. Keep remote path mapping and
authentication in your operations, and treat paths as data rather than inserting
unquoted input into shell commands. Native cancellation stops remaining stages;
your remote backend must define what cancellation means for work already sent.
See [operation contracts](guest_operations.md#replace-file-observations).

For a custom local read-modify-write tool, resolve the actual target and use
`with_file_mutation_queue` around the entire mutation, including its read. It
shares ordering with native edit and write, including existing symlink aliases.
Do not invoke another queued mutation of that same file from inside the callback:
it would wait on itself. Different files can proceed independently. See
[file ordering](guest_operations.md#share-native-file-ordering).

### Bound output and render useful rows

Use [`truncated_tool.rs`](../../examples/extensions/truncated_tool.rs) for a
complete bounded-search tool. The native truncation helpers default to 2,000
lines and 51,200 bytes, stopping at the first limit. Choose head selection for
search/read output and tail selection for logs. A custom tool must apply its own
limits; registering it does not add automatic clipping.

When output is clipped, save complete output before returning its artifact path,
and report omitted counts. Keep large output out of progress snapshots too.
Temporary artifacts can contain private source text and remain on disk; tell the
user where they are and remove them when no longer needed. See
[truncation helpers](guest_operations.md#conversation-and-output) and the
[search example's artifact behavior](../../examples/extensions/README.md#what-the-truncated-search-tool-does).

For custom rows, follow [todo's renderers](../../examples/extensions/todo/view.rs):
keep the collapsed view short, show details on expansion, and distinguish partial
updates from settled results. `render_call` and `render_result` each return a
component. Reuse `last_component` for that slot and `state::<S>()` only for data
shared across the row's two slots. Read current arguments from the render context.
Use `invalidate()` when row-local state changes.

The default shell supplies framing and background. Select `RenderShell::SelfRendered` for `render_shell`
only when your component supplies its own framing. Use the supplied theme and
host width helpers rather than hard-coded colors or byte-count clipping. Format
key hints through the configured keybinding helpers. See
[renderer contracts](guest.md#tool-renderers) and [terminal text helpers](guest_operations.md#columns-and-callbacks).

## Add interaction without requiring a terminal

### Ask, cancel and complete input

Check `has_ui` before depending on an answer. `select`, `input` and `editor`
return no value on dismissal; `confirm` returns false. An empty input answer is
not dismissal. Treat refusal, timeout and cancellation as non-approval rather
than proceeding with a destructive default.

For bounded selection, confirmation or input, pass
`ExtensionUIDialogOptions` with a timeout in milliseconds. The interactive host
shows the countdown and dismisses the dialog when it expires. Pass a signal to
cancel a pending dialog from other work; the guest forwards it and removes its
listener when the dialog settles. Inspect your own signal when you need to
distinguish cancellation from an ordinary no-choice answer. Host rejection is
an error, not a choice. See [dialog options](guest_ui.md#dialogs) and
[interactive UI](../chat/extension-ui.md).

Command argument completion and editor completion solve different tasks. For
slash-command arguments, use `CommandOptions::get_argument_completions`, as in
[`commands.rs`](../../examples/extensions/commands.rs). Return literal candidate
values with readable labels; filter your candidates in the callback.

For syntax elsewhere in the editor, install an autocomplete-provider factory.
Handle your own prefix and delegate unmatched requests, completion insertion and
file-completion decisions to the supplied provider. Preserve its cancellation
signal. Cursor columns are UTF-8 byte offsets, not terminal cells. The host may
rebuild the stack and call the factory again; do not capture a stale editor.
See [completion providers](guest_ui.md#completion-providers).

### Pick a display with the right lifetime

| Need | Start from | Removal or restoration |
| --- | --- | --- |
| Short keyed status | [`status_line.rs`](../../examples/extensions/status_line.rs) | Clear that status key. |
| Working text or indicator | [`working_message_test.rs`](../../examples/extensions/working_message_test.rs) | Pass `None` to restore the host default; an empty frame list hides the indicator. |
| Editor-adjacent lines | [`widget_placement.rs`](../../examples/extensions/widget_placement.rs) | Remove the widget key; absent placement means above the editor. |
| A complete footer | [`custom_footer.rs`](../../examples/extensions/custom_footer.rs) | Restore with `None`; dispose and unsubscribe retained branch listeners. |
| A cancellable custom view | [`qna.rs`](../../examples/extensions/qna.rs) | Close the view and dispose its loader; forward the loader's signal to the work. |

The Q&A example makes a model request in normal use; its controlled tests use a
loopback provider. Loading it is not an offline extraction service.
Show an overlay with `custom` and overlay options; keep its handle while you need
to change visibility or focus. Hiding an overlay is different from closing the
custom operation. A focusable component emits the cursor marker only while
focused and must fit its supplied width. See [custom components](guest_ui.md#custom-components),
[overlay placement](guest_ui.md#shared-overlay-placement) and
[focus](guest_ui.md#authored-focus).

To replace editing, retain the previous factory before installing your own, or
build on the application's `CustomEditor`. Forward unhandled input and the
injected keybindings so interrupt, exit, paste and application actions still work.
Restore with `set_editor_component(None)`. Do not replace the editor merely to
show status. See [editor factories](guest_ui.md#editor-factories).

Register a message renderer for the same custom type you send. Honor expansion,
keep details separate from visible content, and return `None` when the host's
default rendering is sufficient. The first extension in load order registering
that type supplies its renderer. Use the renderer's supplied live theme; semantic
roles describe purpose, not fixed colors. See [custom message components](guest.md#custom-message-components)
and [theme roles](../theme.md).

### Check each supported mode

| Host | What to expect |
| --- | --- |
| Interactive terminal | Dialogs, completion and retained custom components use the terminal UI. |
| RPC | Supported dialogs travel to the client; terminal-only factories are not a remote terminal UI. Follow the [dialog protocol](../rpc.md#extension-dialogs). |
| Print and JSON | No interactive UI. Headless dialogs return no choice or false; display setters do nothing. Use tool results and application events for output. |
| SDK embedding | The embedding chooses its UI host. `has_ui` reports an installed host, not support for every individual method. |

Do not write protocol diagnostics to stdout. A retained UI handle belongs to the
host from which it was acquired; obtain UI again through the current callback
when you need the current session. [Headless results](ui.md#headless-results)
define every fallback.

## Verify before distributing

Use the [existing controlled example runs](../../examples/extensions/README.md#running-an-example)
before trying a real provider. They invoke registered callbacks with supplied
history, UI answers or disposable files, so you can check an extension without
paid calls. The native component tests exercise loading and host bindings; the
in-process adapter alone does not prove those boundaries.

Check refusal and dismissal as well as success. For a stateful tool, change branch
and reload before checking its restored state. For a wrapper, retain the built-in
error, cancellation, truncation and renderer behavior, not only its successful
text result. Keep new example requests with the
[existing example owners](https://github.com/Orchestration-Maestro/maestro/issues/2074).
