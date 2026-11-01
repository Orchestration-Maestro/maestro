# Session file format

A session is a JSONL file: one JSON object per line, each with a `type` member.
Entries other than the header form a tree through `id` and `parentId`, so a
branch is a new child of an earlier entry in the same file. Parsing, context
construction and the operations below are owned by
[session history](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md);
listing and working-directory checks by
[discovery](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/discovery.md).

## Contents

1. [File location](#file-location) and [deleting a session](#deleting-a-session)
2. [Versions](#versions), [messages](#messages) and [entries](#entries)
3. [Tree structure](#tree-structure) and [context construction](#context-construction)
4. [Parsing](#parsing)
5. [Operations](#operations)

## File location

```text
<agent directory>/sessions/--<encoded working directory>--/<timestamp>_<uuid>.jsonl
```

The encoded directory drops one leading `/` or `\`, then replaces every `/`, `\`
and `:` with `-` (`/work/tree` becomes `--work-tree--`). The timestamp is the
header time with `:` and `.` replaced by `-`. An explicit session directory is
used as given. The application resolves the agent directory as described in
[configuration](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/application/configuration.md).
Native writers emit UTF-8 with one compact JSON value and an LF per record.
Native readers replace invalid UTF-8 bytes before parsing rather than rejecting
the file for invalid encoding. Listing does not remove files.

## Deleting a session

To remove a saved session from the terminal picker:

1. Open `/resume` and highlight the session to remove.
2. Press **Ctrl+D** to ask for deletion, even if the search field has text.
3. Press **Enter** to confirm, or **Escape** to return to the list without deleting.

These are the default bindings for `app.session.delete`, `tui.select.confirm`
and `tui.select.cancel`; [keybindings](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/keybindings.md)
can change them. **Ctrl+Backspace** (`app.session.deleteNoninvasive`) also asks
for deletion when the search field is empty; otherwise it edits the query.
The active session cannot be deleted through the picker: Maestro shows
`Cannot delete the currently active session`.

Confirmation authorizes **permanent deletion** if the `trash` program is missing
or fails. Maestro tries `trash` first; if that fails and the file still exists,
it removes the file directly, without asking again. Do not rely on being able
to recover a deleted session.

On success the picker removes the session from its cached lists and refreshes
the visible scope. It shows `Session moved to trash` or `Session deleted`.
A file already absent after the trash attempt is treated as moved to trash.
If direct removal fails, `Failed to delete:` includes the error; the session
stays listed. See the [selector reference](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/chat/session-selector.md#rename-and-delete)
for the refresh and error rules.

You can also delete the session's `.jsonl` file directly with your file manager.
Check its path first: direct file deletion bypasses the picker's active-session
check and trash attempt. Removing a file removes its entire saved tree, not only
the selected branch.

## Versions

The header `version` field selects the layout:

- Version 1: linear entries without `id`/`parentId`.
- Version 2: tree structure linked by `id`/`parentId`.
- Version 3 (current): the `hookMessage` role is named `custom`.

Opening a history through a conversion-capable operation (`open`,
`set_session_file`, `continue_recent`) converts older records in memory; native
persistence then rewrites the file, while an in-memory manager never writes
(see [session history](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md)).
`migrate_session_entries` converts supplied records.
Listing never converts or writes: it reads whatever the file holds. Versions
newer than the current one are left unchanged.

## Messages

Message entries hold one `message` object whose `role` selects its shape.

| Role | Members |
| --- | --- |
| `user` | `content` (string or text/image blocks), `timestamp` (Unix milliseconds) |
| `assistant` | `content` (text, thinking and `toolCall` blocks), `api`, `provider`, `model`, `usage`, `stopReason`, optional `errorMessage`, `timestamp` |
| `toolResult` | `toolCallId`, `toolName`, `content` (text/image blocks), optional `details`, `isError`, `timestamp` |
| `bashExecution` | `command`, `output`, `exitCode` (number or absent), `cancelled`, `truncated`, optional `fullOutputPath`, optional `excludeFromContext`, `timestamp` |
| `custom` | `customType`, `content`, `display`, optional `details`, `timestamp` |
| `branchSummary` | `summary`, `fromId`, `timestamp` |
| `compactionSummary` | `summary`, `tokensBefore`, `timestamp` |

Content blocks: `{"type":"text","text":…}`; `{"type":"image","data":<base64>,"mimeType":…}`;
`{"type":"thinking","thinking":…}`; `{"type":"toolCall","id":…,"name":…,"arguments":{…}}`.
Optional block metadata includes `textSignature` on text, `thinkingSignature`
and `redacted` on thinking, and `thoughtSignature` on tool calls. Assistant
messages may also carry `responseModel`, `responseId` and `diagnostics`. See
[invocation records](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/records.md#wire-records-and-helpers)
for the typed wire rules; the fields above describe the common stored shapes,
not a schema that raw history must pass before loading.
`stopReason` is one of `stop`, `length`, `toolUse`, `error` or `aborted`. `usage`
holds `input`, `output`, `cacheRead`, `cacheWrite`, an optional `totalTokens` and a
`cost` object with `input`, `output`, `cacheRead`, `cacheWrite` and `total`. A
saved history may omit `totalTokens`; the message then still loads, and that
member stays absent until something reports it.
Typed message records live in the
[message projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/messages.md)
owner; stored messages stay raw until a consumer decodes the members it needs.

## Entries

Every entry except the header has `type`, `id` (eight hexadecimal characters
unless a collision forced a full UUID), `parentId` (string, or null for a root)
and `timestamp` (ISO 8601 text).

### Header

The first line carries metadata only: a session `id` but no `parentId`, so it is
not part of the tree. `parentSession` appears when the session was started from
another one.

```jsonl
{"type":"session","version":3,"id":"7f9c2c4e-0f3a-4c9e-9a57-6f0a1d2b3c4d","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/path/to/project","parentSession":"/path/to/original/session.jsonl"}
```

### Message

```jsonl
{"type":"message","id":"a1b2c3d4","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"Hello","timestamp":1733234401000}}
{"type":"message","id":"b2c3d4e5","parentId":"a1b2c3d4","timestamp":"2024-12-03T14:00:02.000Z","message":{"role":"assistant","content":[{"type":"text","text":"Hi!"}],"api":"anthropic-messages","provider":"anthropic","model":"claude-sonnet-4-5","usage":{"input":10,"output":4,"cacheRead":0,"cacheWrite":0,"totalTokens":14,"cost":{"input":0.0,"output":0.0,"cacheRead":0.0,"cacheWrite":0.0,"total":0.0}},"stopReason":"stop","timestamp":1733234402000}}
{"type":"message","id":"c3d4e5f6","parentId":"b2c3d4e5","timestamp":"2024-12-03T14:00:03.000Z","message":{"role":"toolResult","toolCallId":"call_123","toolName":"bash","content":[{"type":"text","text":"output"}],"isError":false,"timestamp":1733234403000}}
```

### Model change

Recorded when the model changes mid-session.

```jsonl
{"type":"model_change","id":"d4e5f6a7","parentId":"c3d4e5f6","timestamp":"2024-12-03T14:05:00.000Z","provider":"openai","modelId":"gpt-4o"}
```

### Thinking level change

```jsonl
{"type":"thinking_level_change","id":"e5f6a7b8","parentId":"d4e5f6a7","timestamp":"2024-12-03T14:06:00.000Z","thinkingLevel":"high"}
```

### Compaction

Stores a summary of earlier messages. `details` (implementation data) and
`fromHook` (true when an extension produced the summary) are optional.

```jsonl
{"type":"compaction","id":"f6a7b8c9","parentId":"e5f6a7b8","timestamp":"2024-12-03T14:10:00.000Z","summary":"User discussed X, Y, Z...","firstKeptEntryId":"c3d4e5f6","tokensBefore":50000}
```

### Branch summary

Summarizes the abandoned path when the selection moves to another branch.
`details` and `fromHook` are optional, as for compaction.

```jsonl
{"type":"branch_summary","id":"a7b8c9d0","parentId":"a1b2c3d4","timestamp":"2024-12-03T14:15:00.000Z","fromId":"f6a7b8c9","summary":"Branch explored approach A..."}
```

### Custom entry

Extension state. It never reaches model context; `customType` identifies the
owner on reload.

```jsonl
{"type":"custom","id":"b8c9d0e1","parentId":"a7b8c9d0","timestamp":"2024-12-03T14:20:00.000Z","customType":"my-extension","data":{"count":42}}
```

### Custom message

Extension-injected content that does reach model context. `content` is a string
or text/image blocks; `display` says whether it is shown; `details` is optional
and not sent to the model.

```jsonl
{"type":"custom_message","id":"c9d0e1f2","parentId":"b8c9d0e1","timestamp":"2024-12-03T14:25:00.000Z","customType":"my-extension","content":"Injected context...","display":true}
```

### Label

A bookmark on another entry. A label record without a `label` member clears it.

```jsonl
{"type":"label","id":"d0e1f2a3","parentId":"c9d0e1f2","timestamp":"2024-12-03T14:30:00.000Z","targetId":"a1b2c3d4","label":"checkpoint-1"}
```

### Session info

Holds the display name; the newest record wins, and an empty name clears it.

```jsonl
{"type":"session_info","id":"e1f2a3b4","parentId":"d0e1f2a3","timestamp":"2024-12-03T14:35:00.000Z","name":"Refactor auth module"}
```

## Tree structure

The first entry has `parentId: null`; an appended entry points at the selected
leaf, which is `null` after the selection is reset, so a history may hold several
roots. Branching moves the selection to an earlier entry before appending.

```text
[user] ── [assistant] ── [user] ── [assistant] ─┬─ [user]             ← leaf
                                                └─ [branch summary] ── [user]
```

## Context construction

`build_session_context` turns the path from the selected leaf into model-visible
messages. It follows parents back to the root and emits that ancestry in
root-to-leaf order, not the order of every record in the file. Model and thinking
changes on that path determine the returned settings; assistant messages also
supply model observations. The latest compaction on
the path contributes its summary, the retained range beginning at
`firstKeptEntryId`, and later messages. Branch summaries and custom messages
contribute content; custom state, labels and session names do not. A hidden
custom message still contributes: `display` controls presentation, not context.

Missing parents stop the walk; repeated entries stop it before a cycle repeats.
The explicit-null leaf selects no entries, while a missing or unknown leaf
falls back to the last supplied entry. See the selection, compaction and
fallback rules in
[session history](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md#branch-selection).

## Parsing

`parse_session_entries` returns raw values and skips malformed lines, so a
reader matches on `type` and reads only the members it needs.

```rust
use maestro_session::parse_session_entries;

let text = concat!(
    r#"{"type":"session","version":3,"id":"s1","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/work"}"#, "\n",
    r#"{"type":"message","id":"a1b2c3d4","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"Hi"}}"#, "\n",
    r#"{"type":"compaction","id":"f6a7b8c9","parentId":"a1b2c3d4","timestamp":"2024-12-03T14:10:00.000Z","summary":"s","firstKeptEntryId":"a1b2c3d4","tokensBefore":50000}"#, "\n",
    r#"{"type":"branch_summary","id":"a7b8c9d0","parentId":"a1b2c3d4","timestamp":"2024-12-03T14:15:00.000Z","fromId":"f6a7b8c9","summary":"x"}"#, "\n",
    r#"{"type":"custom","id":"b8c9d0e1","parentId":"a7b8c9d0","timestamp":"2024-12-03T14:20:00.000Z","customType":"my-extension","data":{"count":42}}"#, "\n",
    r#"{"type":"custom_message","id":"c9d0e1f2","parentId":"b8c9d0e1","timestamp":"2024-12-03T14:25:00.000Z","customType":"my-extension","content":"Injected","display":true}"#, "\n",
    r#"{"type":"label","id":"d0e1f2a3","parentId":"c9d0e1f2","timestamp":"2024-12-03T14:30:00.000Z","targetId":"a1b2c3d4","label":"checkpoint-1"}"#, "\n",
    r#"{"type":"model_change","id":"d4e5f6a7","parentId":"d0e1f2a3","timestamp":"2024-12-03T14:05:00.000Z","provider":"openai","modelId":"gpt-4o"}"#, "\n",
    r#"{"type":"thinking_level_change","id":"e5f6a7b8","parentId":"d4e5f6a7","timestamp":"2024-12-03T14:06:00.000Z","thinkingLevel":"high"}"#, "\n",
);
let show = |value: &serde_json::Value| match value.as_str() {
    Some(text) => text.to_owned(),
    None => value.to_string(),
};
let lines: Vec<String> = parse_session_entries(text)
    .iter()
    .map(|entry| {
        let field = |name: &str| show(&entry[name]);
        match entry["type"].as_str() {
            Some("session") => format!(
                "Session v{}: {}",
                entry.get("version").map_or(String::from("1"), &show),
                field("id")
            ),
            Some("message") => format!(
                "[{}] {}: {}",
                field("id"),
                show(&entry["message"]["role"]),
                entry["message"]["content"]
            ),
            Some("compaction") => format!(
                "[{}] Compaction: {} tokens summarized",
                field("id"),
                field("tokensBefore")
            ),
            Some("branch_summary") => format!("[{}] Branch from {}", field("id"), field("fromId")),
            Some("custom") => format!(
                "[{}] Custom ({}): {}",
                field("id"),
                field("customType"),
                entry["data"]
            ),
            Some("custom_message") => format!(
                "[{}] Extension message ({}): {}",
                field("id"),
                field("customType"),
                field("content")
            ),
            Some("label") => format!(
                "[{}] Label \"{}\" on {}",
                field("id"),
                field("label"),
                field("targetId")
            ),
            Some("model_change") => format!(
                "[{}] Model: {}/{}",
                field("id"),
                field("provider"),
                field("modelId")
            ),
            Some("thinking_level_change") => format!(
                "[{}] Thinking: {}",
                field("id"),
                field("thinkingLevel")
            ),
            _ => String::from("other"),
        }
    })
    .collect();
assert_eq!(
    lines,
    [
        "Session v3: s1",
        "[a1b2c3d4] user: \"Hi\"",
        "[f6a7b8c9] Compaction: 50000 tokens summarized",
        "[a7b8c9d0] Branch from f6a7b8c9",
        "[b8c9d0e1] Custom (my-extension): {\"count\":42}",
        "[c9d0e1f2] Extension message (my-extension): Injected",
        "[d0e1f2a3] Label \"checkpoint-1\" on a1b2c3d4",
        "[d4e5f6a7] Model: openai/gpt-4o",
        "[e5f6a7b8] Thinking: high",
    ]
);
```

## Operations

Creating and opening: `SessionManager::create`, `open`, `continue_recent`,
`in_memory`; switching and resetting: `set_session_file`, `new_session`.
Listing: `SessionManager::list` and `list_all`, described in
[discovery](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/discovery.md).
Extracting a branch into a new history: `create_branched_session`. Forking from
another project's session file: `SessionManager::fork_from`. Their rules are in
[branch extraction and forking](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/sessions/session_manager.md#branch-extraction-and-forking).

Appending, each returning the new entry identity: `append_message`,
`append_thinking_level_change`, `append_model_change`, `append_compaction`,
`append_custom_entry`, `append_session_info`, `append_custom_message_entry`,
`append_label_change` and `branch_with_summary`.

Reading and navigation through `ReadonlySessionManager`: `get_leaf_id`,
`get_leaf_entry`, `get_entry`, `get_branch`, `get_tree`, `get_label`,
`get_entries`, `get_header`, `get_session_name`, `get_cwd`, `get_session_dir`,
`get_session_id`, `get_session_file`. On the manager itself: `get_children`,
`branch`, `reset_leaf` and `build_session_context`. `is_persisted` reports
whether the history is in native-file mode; it does not say the file exists yet,
because a new history publishes only at its first assistant message.
