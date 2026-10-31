# Compaction and branch summarization

Models have limited context windows. Maestro's session library provides two summary
operations, sharing file tracking, message serialization and system instructions:

| Operation | Application trigger | Purpose |
| --- | --- | --- |
| Compaction | Context exceeds threshold, or manual `/compact [instructions]` | Replace old context while retaining recent work |
| Branch summary | `/tree` navigation | Preserve exploration when switching branches |

Commands, event dispatch, navigation and persistence are caller responsibilities, not
session-library operations. The [compaction API](../compaction.md),
[branch API](../branch_summary.md), [session records](session_format.md) and
[extension records](../extensions/guest.md) describe the delivered interfaces.

## Compaction lifecycle

Automatic policy checks `contextTokens > contextWindow - reserveTokens` when enabled.
The shipped reserve is 16384 and retained recent budget is 20000. Manual preparation
is independent of the enabled flag. A response that reports no tokens is measured by
the last real usage plus the messages after it. Applications may pass optional focus
instructions.

1. Find a cut by accumulating estimates backwards through the entries that put a message in context.
2. Extract discarded history and any early split-turn prefix.
3. Generate structured text, using nonempty previous-summary text as iterative context.
4. Append a compaction record containing the returned summary and retained identity.
5. Rebuild context from that summary, the retained precompaction span and later messages.

Only the first three steps are summary-library operations. The session manager owns
append/rebuild. Before/after views, with `u` user, `a` assistant and `t` tool result:

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

Repeated preparation starts at the preceding compaction's `firstKeptEntryId`, not at
the compaction entry itself. If that entry is absent from the path, the range starts
after the preceding compaction. `tokensBefore` is recalculated from the rebuilt
pre-compaction context, not copied from an old estimate.

### Split turns and cuts

A conversational turn begins at a user message, a shell execution or a generated message
(custom, branch summary or compaction summary) and continues through assistant/tool
work until the next such message. A large turn can require a mid-turn cut:

```text
  header | u1 a2 t3 a4 t5 t6 | a7 t8
           ^ turn start      ^ first kept
           prefix (1..6)       retained (7..8)

  messagesToSummarize = []
  turnPrefixMessages = [u1, a2, t3, a4, t5, t6]
  isSplitTurn = true
```

History and prefix summaries start independently, then merge with a **Turn Context
(split turn)** separator. Empty history for a split with a nonempty prefix uses
`No prior history.` without a history request. Prefix instructions do not inherit
history focus or previous-summary text. See the API for their distinct output budgets.

Cuts may start at user, assistant, shell execution, custom-message or branch-summary
content, never at a tool result. Custom messages and summaries count toward the retained
budget like any other message, and an empty branch summary is neither a cut nor a turn
start because it puts nothing in context. Preceding metadata is absorbed without
crossing a message, another context entry or a compaction boundary.

### Stored compaction record

Records contain `type: "compaction"`, `id`, nullable `parentId`, ISO string `timestamp`,
`summary`, `firstKeptEntryId`, `tokensBefore` and optional `fromHook`/`details`.
The [entry owner](../entry.md) supplies Rust record types; stored histories remain raw
JSON until selected fields are consumed. Built-in `CompactionDetails` contains
`readFiles` and `modifiedFiles`. Extensions may store other JSON details, so built-in
preparation does not treat hook-generated details as built-in file lists.

## Branch lifecycle

An application can offer a summary when navigating away from exploration. Collection
finds the deepest shared ancestor and excludes it from the abandoned entries; existing
compactions remain content rather than stopping traversal. Preparation retains newest
content under context-window-minus-reserve; the [branch API](../branch_summary.md)
specifies the summary exception, defaults and request/result policy.

1. Collect the abandoned path after the common ancestor.
2. Let caller hooks cancel, supply a custom summary or override instructions.
3. Generate the default summary only when neither cancellation nor a custom summary
   bypasses generation; instruction overrides configure that generation.
4. Publish the summary at the selected navigation point using the session operation.

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

The summary attaches to the selected target, not to the abandoned leaf. The existing
`branch_with_summary` operation records the selected point as `fromId` (or literal
`root` for no selected parent); do not infer an abandoned-leaf identity from its name.
A branch record contains `type: "branch_summary"`, `id`, nullable `parentId`, ISO string
`timestamp`, `summary`, `fromId` and optional `fromHook`/`details`. Built-in
`BranchSummaryDetails` has the same two file arrays as compaction details. Extension
details may instead contain application-defined JSON.

### Cumulative file tracking

Compaction carries only the preceding built-in compaction's file details, then extracts
assistant tool paths from discarded history and split prefixes, not the kept suffix.
Branch preparation scans all built-in branch-summary details, even outside its retained
message budget, then extracts paths from visited assistant messages before each budget
check. It does not carry compaction details. These different sources retain cumulative
tracking across repeated compactions or nested branch summaries without mixing policies.
Shared helpers remove modified paths from read-only lists and append optional sections:

```text
<read-files>
path/to/read.rs
</read-files>

<modified-files>
path/to/changed.rs
</modified-files>
```

## Summary formats and prompts

History and branch prompts both request Goal, Constraints & Preferences, Progress
(Done, In Progress, Blocked), Key Decisions and Next Steps. History additionally asks
for Critical Context. Branch instructions refer specifically to exploration in that
branch; they are not the history prompt. Updates merge the prior summary rather than
starting over. The model writes section contents; the library does not parse or enforce
the requested Markdown structure.

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

Branch success prefixes generated text with:

```text
The user explored a different conversation branch before returning here.
Summary of that exploration:

```

Requests use the shared summarization system instruction, not a continuation of the
original conversation. The complete serialized conversation is wrapped in
`<conversation>` tags. History updates also supply `<previous-summary>` tags. See the
API pages for exact instruction selection and error/abort behavior, which differ by
operation; custom replacement instructions are a branch-only control.

### Message serialization

Use `convert_to_llm` before `serialize_conversation` when working with typed application
messages. The built-in generators select raw historical fields through the same shared
projection/writer, preserving unread metadata without decoding execution records.

```text
[User]: What they said
[Assistant]: Response text
[Assistant tool calls]: read(path="foo.rs"); edit(path="bar.rs")
[Tool result]: Output from tool
```

Serialization separates labels with blank lines and groups assistant text and tools in
that order. Assistant thinking is never written, in compaction, branch summaries or the
exported conversation text. Arguments pair by name for object roots; array and string roots
pair their positions instead (`echo(0=1, 1=2)`), and other roots name none. Only
tool-result text is clipped at 2000 Unicode scalar values,
with an omitted-character notice; user and assistant text stay complete. Branch
preparation excludes tool results altogether, so their context comes from tool calls.

## Extension integration

The guest [event and result API](../extensions/guest.md) defines
`SessionBeforeCompactEvent`, `SessionBeforeCompactResult`, `SessionBeforeTreeEvent`
and `SessionBeforeTreeResult`. Applications dispatch these hooks and supply their
cancellation signals; the session library does not dispatch extensions.

### Before compaction

`session_before_compact` can cancel or return custom compaction. Its preparation holds
`messages_to_summarize`, `turn_prefix_messages`, `is_split_turn`, `previous_summary`,
`file_ops`, `tokens_before`, `first_kept_entry_id` and `settings`. The event also exposes
branch entries and optional custom instructions for application-specific state.

A handler may build either result below using guest-owned types:

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

For a custom model, convert the prepared messages and serialize them before sending
them. Return the generated summary with the preparation's retained ID and token count.
The [guest compaction example](../extensions/guest.md#a-user-request-summary-handler)
shows registration and callback context; it does not choose session settings.

### Before tree navigation

`session_before_tree` is an application hook even when no summary was requested.
Its preparation holds `target_id`, `old_leaf_id`, `common_ancestor_id`,
`entries_to_summarize` and `user_wants_summary`. A handler can cancel navigation or
supply summary/details when the caller selected summarization:

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

These snippets describe callback results, not a new session dispatch API. See the
[guest handler documentation](../extensions/guest.md) for registration and full types.

## Settings

The [settings guide](settings.md#compaction-and-branch-summaries) describes user/project
files and merge behavior. The shipped automatic policy is:

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
manual `/compact` action. Branch reserve is configured separately. Local user overrides
are not library defaults.
