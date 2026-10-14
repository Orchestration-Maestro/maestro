# Compaction preparation

`maestro_session::compaction` prepares summary input from a supplied session branch,
without requesting a model, mutating history or persisting entries. Its implemented
items are also re-exported at the crate root. These synchronous operations are available
on native and browser targets. Session traversal and storage remain with the
[session manager](sessions/session_manager.md).

## Prepare a branch

Supply raw `SessionEntry` values in branch order with usable IDs and parent links.
`prepare_compaction` returns owned history and split-turn prefix messages, the first
retained ID, tokens-before, previous summary, file operations and settings. It returns
`None` when the branch ends in compaction or lacks a usable first-kept ID. Reached
invalid selected fields return `serde_json::Error`; unrelated raw fields remain intact.
An empty previous summary stays present. Manual preparation is not gated by the
settings' automatic-compaction trigger.

```rust
use maestro_models::Message;
use maestro_session::{CompactionSettings, DEFAULT_COMPACTION_SETTINGS,
    prepare_compaction, serialize_conversation};
use serde_json::{Value, json};

let entries = vec![
    json!({"type":"message", "id":"old", "parentId":null,
        "message":{"role":"user", "content":"earlier", "timestamp":0}}),
    json!({"type":"message", "id":"recent", "parentId":"old",
        "message":{"role":"user", "content":"latest", "timestamp":0}}),
];
let settings = CompactionSettings {
    keep_recent_tokens: 1.0,
    ..DEFAULT_COMPACTION_SETTINGS
};
let prepared = prepare_compaction(&entries, &settings)?.unwrap();
assert_eq!(prepared.first_kept_entry_id, "recent");
// These selected messages are already model-facing user messages.
let messages: Vec<Message> = serde_json::from_value(Value::Array(prepared.messages_to_summarize))?;
assert_eq!(serialize_conversation(&messages)?, "[User]: earlier");
# Ok::<(), serde_json::Error>(())
```

## Estimates and safe cuts

`DEFAULT_COMPACTION_SETTINGS` enables compaction, reserves 16384 tokens and keeps
20000 recent tokens. `should_compact` requires usage strictly greater than the context
window minus reserve; disabled settings never trigger it. `calculate_context_tokens`
uses a nonzero reported total or adds input, output, cache-read and cache-write counts.
`estimate_context_tokens` selects the latest successful assistant usage, ignoring
error/aborted attempts, and estimates subsequent messages; without usage it estimates
all supplied messages.

`estimate_tokens` rounds up UTF-16 code units divided by four. It includes assistant
text, thinking and canonical JSON tool arguments, shell command/output even when
excluded from model context, and unwrapped summary text. User images add nothing;
custom/tool-result images add 4800 units each. Unknown roles contribute zero.

`find_cut_point` accepts an existing half-open index range. It counts only message
entries backwards, crosses at the retained budget and selects the next valid cut,
falling back to the first candidate. Tool results cannot start cuts. Preceding metadata
is absorbed without crossing a message or compaction boundary. `find_turn_start_index`
distinguishes user/shell message starts from custom-message/branch-summary entry starts.
A non-user cut with a turn start produces a split prefix, which may be empty.

## Shared summary input

`create_file_ops` and `extract_file_ops_from_message` track nonempty string paths on
assistant read/write/edit calls. Preparation tracks history and split prefixes, not
the kept suffix, carrying previous built-in file details only when `fromHook` is falsy.
`compute_file_lists` removes modified paths from reads, unions edits/writes and sorts
both lists in UTF-16 lexical order. `format_file_operations` preserves supplied list
order and duplicates in read-files/modified-files sections.

`serialize_conversation` accepts typed model messages, not raw application messages.
Project application messages through the [message conversion interface](sessions/messages.md)
first. It emits user, assistant-thinking, assistant, assistant-tool-call and tool-result
labels, with blank-line paragraph separators. Tool arguments use canonical JSON key
and number spelling; assistant categories appear in thinking/text/tool order.
Only tool-result text is clipped at 2000 Unicode scalar values with an omitted-character
notice. User and assistant text remain complete; stored content is not rewritten.

Summary generation, model requests and application compaction orchestration are not
part of this preparation interface.
