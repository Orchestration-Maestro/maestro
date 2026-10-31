# Session operations

[`AgentSession`](../../crates/maestro-app/src/agent_session/mod.rs) is the session
every frontend shares. `AgentSession::new` takes an `AgentSessionConfig` and moves
the supplied owners in: the local agent, the session history, the settings, the
working directory, the scoped models, the resource loader, the model catalog and
the command port that configured credentials use. It subscribes to the agent's
events, installs the tool hooks, then builds the extension runner, the tool
registry and the base system prompt. It fails when that prompt cannot find its
documentation path or an extension error listener fails. Programs usually create
it through [`create_agent_session`](sdk.md), which also covers the bindings,
the reload and session replacement.

Session state lives in one cell. No borrow of that cell, the history, the settings
or the agent state is held while the session awaits work, calls a listener or an
extension event handler, or resolves a tool description, so any of them can call
back into the session.

## Agent events

The session's one agent listener opens the retry observation at once when a run
ends on a response a retry can recover, then queues the event. Queued events are
processed one at a time in order; a failing event is dropped and the rest still
run. For each event the session first removes a started user message's text from
the steering mirror, else the follow-up mirror, announcing the queues. It then
tells the extensions, then the listeners. A finished custom message is stored as
a custom entry, and a finished user, assistant or tool-result message as a
message entry. When the run ends, the newest response is retried when it can be,
else checked for compaction. An extension that rewrites a finished message
replaces it in the shared history, so the listeners, later events, the totals
and the stored entry read the replacement. Extension turn events count turns
from the run's start.

The session tells its listeners of every agent event in one place, and reads the
`holdUncheckedText` setting there ([settings](../settings.md#delivery-and-transport)).
While it is `true`, the listeners hear no text or thinking delta, and the start and
tool-call updates of an assistant message reach them with their text and thinking
blocks emptied. A tool's progress reaches them without its content or details.
Tool-call and status events, the message end and the completed tool result arrive as
they are. The extensions are told first and see the events whole.

A tool call reaches the `tool_call` handlers only after the queued events are
processed, and the arguments they edit are the ones executed. `tool_result`
handlers may replace a result's content, details and error flag. A manual
compaction keeps agent events from the session until it ends.

## Observations and listeners

The getters read the supplied owners live: the agent state, model, thinking level,
streaming flag and system prompt, the steering and follow-up modes, the session
file, identity and name, the scoped models and the loaded prompt templates.
`messages()` returns the agent's live history handle, not a copy, and
`resource_loader()` returns the loader the session was given, so a caller reads
its skills, prompt templates, extensions and context files from the same object.

`subscribe` adds a listener and returns the operation that removes it; removing
twice does nothing. Listeners run in subscription order, including ones added
while an event is delivered. Subscribing the same listener twice keeps one
registration.

## Models and thinking

`set_model` refuses a model without configured credentials with
`No API key for <provider>/<id>` and changes nothing. Otherwise it picks the
starting thinking level: the current level, or, when the current model cannot
reason, the saved default level (`medium` when none is saved). It then selects
the model, records a `model_change` entry, saves the default provider and model,
applies the level and tells extensions through `model_select` unless provider and
identity are unchanged. It writes the entry and the saved default even when the same
model is selected again.

The `model_select` notification starts at once and finishes on the session's task
facility, so a caller that stops waiting does not cancel it and a caller that keeps
waiting returns after the handlers finish. `cycle_model` notifies the same way.

`cycle_model` moves forward or backward through the scoped models that have
credentials, or through every available model when none are scoped. Fewer than
two candidates return `None`. A current model that is not listed starts from the
first position. A scoped entry's own level wins over the starting level.

`set_thinking_level` keeps a supported level and clamps any other to the model.
It always writes the agent level. Only an effective change records a
`thinking_level_change` entry, saves the default level (except `off` on a model
that cannot reason), emits `ThinkingLevelChanged` and starts `thinking_level_select`.
That notification starts at once and finishes on the session's task facility, so a
slow handler never delays the change. An unrecognized saved level reads as
`off` and clamps to the model's first supported level.

`cycle_thinking_level` returns `None` for a model that cannot reason and otherwise
moves to the next supported level, wrapping. `set_steering_mode` and
`set_follow_up_mode` return `Result<(), DiagnosticErrorInfo>`. They admit the
preference edit before changing the agent's queue mode, then save it. A refusal
changes neither value and reaches RPC as an unsuccessful response.
`set_transport` admits and saves the transport before updating the next request's
live preference.

SDK model, thinking and explicit tool options are admitted before initial history
entries are appended. Recorded model and thinking values instead use stored-value
resolution. A model that resolves to a different identity takes the ordinary
fallback path and message; thinking resolves before the model's capability clamp.
Resolution reports settings diagnostics without rewriting the saved session.

## Tools and the system prompt

Every runtime build and every extension refresh rebuilds the tool registry: the
built-in definitions first, under `<builtin:name>`, then the extension and SDK
tools replacing them by name, keeping only allowlisted names when there is an
allowlist. For each definition the registry keeps the prompt snippet with
whitespace runs collapsed to one space (dropping blank snippets) and the prompt
guidelines trimmed, without blanks or repeats, and wraps one executable tool. It
then activates tools and rebuilds the system prompt. A runtime build activates
the initial tools (`read`, `bash`, `edit` and `write` by default) and adds every
extension and SDK tool; a refresh keeps the active tools and adds the tools new
to the registry. With an allowlist, every allowed registry tool is active.
Repeated names are dropped.

`set_active_tools_by_name` keeps the registered names in the given order, skipping
unknown names and keeping repeats. It sets the agent tools and rebuilds the system
prompt from the working directory, the loaded skills and context files, the
loader's replacement prompt, the appended texts joined by a blank line, the
selected names, their snippets and all their guidelines in order. That prompt and
the options that built it become the base prompt that prompts restore.
`get_all_tools` lists the registry with each tool's source.

Explicit active-tool selection asks the registered decider before changing tools
or the prompt. Automatic startup and reload ask it for each candidate, retain
refused definitions in the registry and leave them inactive with a diagnostic.
Without a decider, the active set and its ordering are unchanged.

## Prompts, queues and shell commands

`prompt`, `steer`, `follow_up`, `send_custom_message`, `send_user_message`,
`clear_queue` and `execute_bash` are described in
[prompts, queues and shell commands](prompts.md).

## Names and navigation

`set_session_name` records a `session_info` entry and announces the trimmed name,
or no name when it is empty.

Calling `navigate_tree` reads the target and starts the before-tree operation
before the returned future is polled. Dropping even a never-polled result future
leaves the move running; `abort_branch_summary` stops its summary. It does nothing
when the target is already the leaf and refuses an unknown target with
`Entry <id> not found`. Extensions handling
`session_before_tree` receive the branch left behind and may cancel the move,
supply the summary (used only when a summary was requested), or override the
instructions and the label; a null override clears it. Otherwise, when a summary
was requested and the branch left has entries, the summarizer runs with the
current model and the saved branch-summary reserve. An aborted summary cancels
the move; a failed one returns its message.

A user or custom message target moves the leaf to its parent and returns the
message text for the editor; any other entry becomes the leaf. A non-empty summary
is stored as a branch summary with its read and modified files, and the label goes
on that entry; otherwise the label goes on the target. The agent history is then
rebuilt from the session context and extensions receive `session_tree`.

A summary request without a usable key fails with the login help for the
provider, or, for an expired subscription, asks the user to log in again. Any other
credential failure is returned verbatim.

`get_user_messages_for_forking` lists the user messages that have text, in entry
order.

## Compaction and retries

`compact` shrinks the context on demand, and the session compacts on its own when
a run ends past the reserve; a failed response is repeated after a doubling delay.
Both announce their start and end events, can be aborted, and are described with
the preferences they read in [compaction and retries](recovery.md).
`set_auto_compaction_enabled` and `set_auto_retry_enabled` return
`Result<(), DiagnosticErrorInfo>` and admit their edits before publishing them.
RPC reports a refusal as unsuccessful; a refused compaction edit leaves the
terminal footer unchanged.

## Statistics

`get_session_stats` counts messages by role and tool calls, and sums tokens and
cost over the answers. `get_context_usage` has no value without a context window.
After a compaction, the tokens stay unknown until the newest answer that was
neither aborted nor failed reports usage. `get_last_assistant_text` joins the text
blocks of the newest answer, skipping an aborted empty one, and trims the result.

## Exports

`export_to_html` writes the session through the export owner with the agent's
system prompt and tools, the saved theme name and custom tool rendering. The
caller supplies the export assets and the rendering environment whose published
theme the renderers use. A renderer resolves that theme only when it reads a
color: without a published theme, one that never reads it still draws, and one
that reads it keeps its standard rendering. Renderers run with the session free,
so one may change it. It refuses a session that is only in memory before it
renders or writes anything.

`export_to_jsonl` writes a fresh session header followed by the current branch,
each entry's `parentId` replaced by the previous entry's identity. Without a path
the file is `session-<time>.jsonl` in the process working directory, where `<time>`
is the current UTC time with `:` and `.` replaced by `-`. The export creates
missing parent directories and returns the resolved path.
