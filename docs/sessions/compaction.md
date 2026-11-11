# Compaction and branch summarization

Models have limited context windows. Maestro's session library offers two summary
operations. They share file tracking, message serialization and system
instructions:

| Operation | Application trigger | Purpose |
| --- | --- | --- |
| Compaction | Context exceeds threshold, or manual `/compact [instructions]` | Replace old context while retaining recent work |
| Branch summary | `/tree` navigation | Preserve exploration when switching branches |

Commands, event dispatch, navigation and persistence belong to the caller, not to
the session library. The [compaction API](../compaction.md),
[branch API](../branch_summary.md), [session records](session_format.md) and
[extension records](../extensions/guest.md) describe the interfaces Maestro
delivers. For complete-session calls, persistence and events, use the
[application compaction guide](../application/compaction.md).

## Compaction lifecycle

When automatic compaction is enabled, the policy checks
`contextTokens > contextWindow - reserveTokens`. The shipped reserve is 16384 and
the retained recent budget is 20000. Manual preparation does not depend on the
enabled flag. When a response reports no tokens, Maestro measures the context from
the last real usage plus the messages after it. With no real usage at all, the
automatic check starts no compaction. Applications may pass optional focus
instructions.

Each user, custom or tool-result image adds 4800 UTF-16 units to the estimate,
which is 1200 tokens. When Maestro picks the usage anchor, it skips all-zero
assistant usage. Usage from before a compaction summary inserted ahead of the
retained span is stale, and Maestro ignores it until a new answer reports usage.

1. Find a cut by accumulating estimates backwards through the entries that put a
   message in context.
2. Extract the discarded history and any early split-turn prefix.
3. Generate structured text, with nonempty previous-summary text as iterative
   context.
4. Append a compaction record holding the returned summary and the retained
   identity.
5. Rebuild the context from that summary, the retained precompaction span and the
   later messages.

Only the first three steps belong to the summary library. The session manager owns
the append and the rebuild. The views before and after, where `u` is a user
message, `a` an assistant message and `t` a tool result:

```text
Before:
  header | u1 a2 t3 | u4 a5 t6 t7 a8 t9
           discarded  retained from u4

After append:
  header | u1 a2 t3 | u4 a5 t6 t7 a8 t9 | compaction10
           not sent  retained from u4

Model context:
  system | summary from compaction10 | u4 a5 t6 t7 a8 t9
```

Repeated preparation starts at the preceding compaction's `firstKeptEntryId`, not
at the compaction entry itself. If that entry is absent from the path, the range
starts after the preceding compaction. Maestro recalculates `tokensBefore` from the
rebuilt pre-compaction context and never copies an old estimate.

### Split turns and cuts

A conversational turn begins at a user message, a shell execution or a generated
message (custom, branch summary or compaction summary). It continues through the
assistant and tool work until the next such message. Stored compaction entries are
neither turn starts nor cut candidates, but their summary messages still count
toward the budget. A large turn can require a mid-turn cut:

```text
  header | u1 a2 t3 a4 t5 t6 | a7 t8
           ^ turn start      ^ first kept
           prefix (1..6)       retained (7..8)

  messagesToSummarize = []
  turnPrefixMessages = [u1, a2, t3, a4, t5, t6]
  isSplitTurn = true
```

The history and prefix summaries start independently, then merge under a
**Turn Context (split turn)** separator. When a split has a nonempty prefix but an
empty history, Maestro uses `No prior history.` and sends no history request.
Prefix instructions inherit neither the history focus nor the previous-summary
text. The API page describes their separate output budgets.

A cut may start at user, assistant, shell execution, custom-message or
branch-summary content, and never at a tool result. Custom messages and summaries
count toward the retained budget like any other message. An empty branch summary
is neither a cut nor a turn start, because it puts nothing in context. Preceding
metadata is absorbed, but never across a message, another context entry or a
compaction boundary.

### Stored compaction record

A compaction record holds `type: "compaction"`, `id`, a nullable `parentId`, an ISO
string `timestamp`, `summary`, `firstKeptEntryId`, `tokensBefore` and the optional
`fromHook` and `details`. The [entry owner](../entry.md) supplies the Rust record
types; stored histories stay raw JSON until Maestro consumes selected fields.
Built-in `CompactionDetails` holds `readFiles` and `modifiedFiles`. Extensions may
store other JSON details, so built-in preparation does not treat hook-generated
details as built-in file lists.

## Branch lifecycle

An application can offer a summary when the user navigates away from an
exploration. Collection finds the deepest shared ancestor and leaves it out of the
abandoned entries. Existing compactions stay in as content and do not stop
traversal. Preparation keeps the newest content that fits under the context window
minus the reserve. The [branch API](../branch_summary.md) specifies the summary
exception, the defaults and the request and result policy.

1. Collect the abandoned path after the common ancestor.
2. Let caller hooks cancel, supply a custom summary or override the instructions.
3. Generate the default summary only when neither a cancellation nor a custom
   summary bypasses generation; instruction overrides configure that generation.
4. Publish the summary at the selected navigation point with the session
   operation.

```text
Before:
       / B -- C -- D (old leaf)
  A --<
       \ E -- F (target)

  ancestor: A; collected: B, C, D

After branch_with_summary(Some("F"), ...):
       / B -- C -- D
  A --<
       \ E -- F -- summary (new leaf)
```

The summary attaches to the selected target, not to the abandoned leaf. The
existing `branch_with_summary` operation records the selected point as `fromId`
(or the literal `root` when no parent is selected); do not infer an abandoned-leaf
identity from its name. A branch record holds `type: "branch_summary"`, `id`, a
nullable `parentId`, an ISO string `timestamp`, `summary`, `fromId` and the
optional `fromHook` and `details`. Built-in `BranchSummaryDetails` has the same two
file arrays as compaction details. Extension details may hold any
application-defined JSON instead.

### Cumulative file tracking

Compaction carries only the preceding built-in compaction's file details. It then
extracts assistant tool paths from the discarded history and split prefixes, not
from the kept suffix. Branch preparation scans every built-in branch-summary
details record, even outside its retained message budget, then extracts paths from
the assistant messages it visits before each budget check. It does not carry
compaction details. Each operation keeps its own source, so tracking stays cumulative
across repeated compactions and nested branch summaries without mixing policies.
Shared helpers remove modified paths from the read-only lists and append the
optional sections:

```text
<read-files>
path/to/read.rs
</read-files>

<modified-files>
path/to/changed.rs
</modified-files>
```

## Summary formats and prompts

The history and branch prompts both request Goal, Constraints & Preferences,
Progress (Done, In Progress, Blocked), Key Decisions and Next Steps. The history
prompt also asks for Critical Context. Branch instructions refer specifically to
the exploration in that branch and are not the history prompt. An update merges the
prior summary instead of starting over. The model writes the section contents; the
library neither parses nor enforces the requested Markdown structure.

```markdown
## Goal
[What the user is trying to accomplish]

## Constraints & Preferences
- [Requirements]

## Progress
### Done
- [x] [Completed work]
### In Progress
- [ ] [Current work]
### Blocked
- [Issues]

## Key Decisions
- **[Decision]**: [Rationale]

## Next Steps
1. [What happens next]

## Critical Context
- [History-only continuation details]
```

On success, a branch summary starts with this prefix before the generated text:

```text
The user went down another branch of this conversation and then came back to this point.
Summary of that branch:

```

Requests use the shared summarization system instruction, not a continuation of the
original conversation. The complete serialized conversation is wrapped in
`<conversation>` tags, and history updates also supply `<previous-summary>` tags.
Instruction selection and error and abort behavior differ by operation; the API
pages give the exact rules. Custom replacement instructions are a branch-only
control.

### Message serialization

Call `convert_to_llm` before `serialize_conversation` when you work with typed
application messages. The built-in generators select raw historical fields through
the same shared projection and writer. They keep unread metadata and never decode
execution records.

```text
[User]: What they said
[Assistant]: Response text
[Assistant tool calls]: read(path="foo.rs"); edit(path="bar.rs")
[Tool result]: Output from tool
```

Serialization separates labels with blank lines and groups assistant text and tools
in that order. It never writes assistant thinking, whether for compaction, branch
summaries or the exported conversation text. Arguments pair by name for object
roots; array and string roots pair their positions instead (`echo(0=1, 1=2)`), and
other roots name none. Only tool-result text is clipped, at 2000 Unicode scalar
values, with an omitted-character notice; user and assistant text stay complete.
Branch preparation leaves tool results out altogether, so their context comes from
the tool calls.

`generate_summary_with_usage` returns `GeneratedSummary`, which holds the summary
text and the `Usage` the provider reported. `generate_summary` still returns the
text alone. Neither helper stores usage on a session entry. The
[summary API](../compaction.md#generate-a-summary) lists the request options.

## Extension integration

The guest [event and result API](../extensions/guest.md) defines
`SessionBeforeCompactEvent`, `SessionBeforeCompactResult`, `SessionBeforeTreeEvent`
and `SessionBeforeTreeResult`. Applications dispatch these hooks and supply their
cancellation signals; the session library does not dispatch extensions.

### Before compaction

`session_before_compact` can cancel compaction or return a custom one. Its
preparation holds `messages_to_summarize`, `turn_prefix_messages`, `is_split_turn`,
`previous_summary`, `file_ops`, `tokens_before`, `first_kept_entry_id` and
`settings`. The event also exposes the branch entries and any custom instructions,
for application-specific state.

A handler can build either result below with guest-owned types:

```rust,ignore
use maestro_extensions_wasm::{CompactionResult, Presence, SessionBeforeCompactResult};

let cancel = SessionBeforeCompactResult {
    cancel: Presence::Present(true),
    compaction: Presence::Missing,
};
let custom = SessionBeforeCompactResult {
    cancel: Presence::Missing,
    compaction: Presence::Present(CompactionResult {
        summary: "Custom summary".into(),
        first_kept_entry_id: preparation.first_kept_entry_id.clone(),
        tokens_before: preparation.tokens_before,
        details: Presence::Missing,
    }),
};
```

For a custom model, convert the prepared messages and serialize them before you
send them. Return the generated summary with the preparation's retained ID and
token count. The
[guest compaction example](../extensions/guest.md#a-user-request-summary-handler)
shows registration and callback context; it does not choose session settings.

### Before tree navigation

`session_before_tree` is an application hook even when no summary was requested.
Its preparation holds `target_id`, `old_leaf_id`, `common_ancestor_id`,
`entries_to_summarize` and `user_wants_summary`. A handler can cancel navigation,
or supply a summary and details when the caller selected summarization:

```rust,ignore
use maestro_extensions_wasm::{Presence, SessionBeforeTreeResult, SessionBeforeTreeSummary};

let cancel = SessionBeforeTreeResult {
    cancel: Presence::Present(true),
    summary: Presence::Missing,
    custom_instructions: Presence::Missing,
    replace_instructions: Presence::Missing,
    label: Presence::Missing,
};
if preparation.user_wants_summary {
    let custom = SessionBeforeTreeResult {
        summary: Presence::Present(SessionBeforeTreeSummary {
            summary: "Exploration summary".into(),
            details: Presence::Missing,
        }),
        cancel: Presence::Missing,
        custom_instructions: Presence::Missing,
        replace_instructions: Presence::Missing,
        label: Presence::Missing,
    };
}
```

These snippets show callback results; they add no session dispatch API. The
[guest handler documentation](../extensions/guest.md) covers registration and the
full types.

## Settings

The [settings guide](settings.md#compaction-and-branch-summaries) describes the
user and project files and how they merge. The shipped automatic policy is:

```json
{
  "compaction": {
    "enabled": true,
    "reserveTokens": 16384,
    "keepRecentTokens": 20000
  }
}
```

| Setting | Default | Meaning |
| --- | --- | --- |
| `enabled` | `true` | Automatic compaction policy |
| `reserveTokens` | `16384` | Space reserved for prompt and response |
| `keepRecentTokens` | `20000` | Recent context retained without summarizing |

Disabling automatic compaction does not disable manual preparation or a caller's
manual `/compact` action. Branch reserve is configured separately. Local user
overrides are not library defaults.
