# Session operations

[`AgentSession`](../../crates/maestro-app/src/agent_session/mod.rs) is the session
every frontend shares. `AgentSession::new` takes an `AgentSessionConfig` and moves
the supplied owners in: the local agent, the session history, the settings, the
working directory, the scoped models, the resource loader, the model catalog, the
extension runner and the command port that configured credentials use. It
subscribes to nothing and builds no runtime of its own.

Session state lives in one cell. No borrow of that cell, the history, the settings
or the agent state is held while the session awaits work or calls a listener or an
extension handler, so a handler can call back into the session.

## Observations and listeners

The getters read the supplied owners live: the agent state, model, thinking level,
streaming flag and system prompt, the steering and follow-up modes, the session
file, identity and name, the scoped models and the loaded prompt templates.
`messages()` returns the agent's live history handle, not a copy.

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

`cycle_model` moves forward or backward through the scoped models that have
credentials, or through every available model when none are scoped. Fewer than
two candidates return `None`. A current model that is not listed starts from the
first position. A scoped entry's own level wins over the starting level.

`set_thinking_level` keeps a supported level and clamps any other to the model.
It always writes the agent level. Only an effective change records a
`thinking_level_change` entry, saves the default level (except `off` on a model
that cannot reason), emits `ThinkingLevelChanged` and starts `thinking_level_select`.
That notification starts at once and finishes on the agent's task facility, so a
slow handler never delays the change. An unrecognized saved level reads as `off`
and clamps to the model's first supported level.

`cycle_thinking_level` returns `None` for a model that cannot reason and otherwise
moves to the next supported level, wrapping. `set_steering_mode` and
`set_follow_up_mode` set the agent's queue mode and save it.

## Tools and the system prompt

`set_tool_definitions` takes the registry in order. For each definition it keeps
the prompt snippet with whitespace runs collapsed to one space (dropping blank
snippets) and the prompt guidelines trimmed, without blanks or repeats, and wraps
one executable tool.

`set_active_tools_by_name` keeps the registered names in the given order, skipping
unknown names and keeping repeats. It sets the agent tools and rebuilds the system
prompt from the working directory, the loaded skills and context files, the
loader's replacement prompt, the appended texts joined by a blank line, the
selected names, their snippets and all their guidelines in order. `get_all_tools`
lists the registry with each tool's source.

## Names and navigation

`set_session_name` records a `session_info` entry and announces the trimmed name,
or no name when it is empty.

`navigate_tree` does nothing when the target is already the leaf and refuses an
unknown target with `Entry <id> not found`. Extensions handling
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
theme the renderers use. It refuses a session that is only in memory.

`export_to_jsonl` writes a fresh session header followed by the current branch,
each entry's `parentId` replaced by the previous entry's identity. Without a path
the file is `session-<time>.jsonl` in the process working directory, where `<time>`
is the current UTC time with `:` and `.` replaced by `-`. The export creates
missing parent directories and returns the resolved path.
