# Compaction

`maestro_session::compaction` prepares summary input and generates history or split-turn
summaries without mutating history or persisting entries.
[Abandoned-branch generation](branch_summary.md) is a separate interface with its own
budget and result policy. The [complete guide](sessions/compaction.md) covers retained
spans, navigation, formats, settings and caller-owned extension integration. Its operations are also
re-exported at the crate root and available on native and browser targets. Session traversal and storage remain with the
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
uses a nonzero reported total, treating an omitted or null total as absent, or
adds input, output, cache-read and cache-write counts.
`estimate_context_tokens` selects the latest successful assistant usage, ignoring
error/aborted attempts, and estimates subsequent messages; without usage it estimates
all supplied messages.

`estimate_tokens` rounds up UTF-16 code units divided by four. It includes assistant
text, thinking and canonical JSON tool arguments, shell command/output even when
excluded from model context, and unwrapped summary text. Each image in a user, custom
or tool-result message adds 4800 units. Unknown roles contribute zero.

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

## Generate a summary

`generate_summary` accepts raw application messages, a supplied model, reserve tokens,
an optional prior summary and `SummaryOptions`. It reads only summary-visible fields,
using the [message projection](sessions/messages.md) formatting and the shared serializer
above. It starts the registered model completion before returning an owned future;
borrowed inputs need not outlive that future. A fresh request timestamp is used.

Supply the API key explicitly (empty is retained), optional ordered headers and shared
cancellation. Reasoning is omitted unless the model supports it and an enabled level
is explicitly selected. No thinking level is chosen by default. Nonempty prior text
selects update instructions; nonempty custom instructions add history focus. Whitespace
counts as nonempty. The history budget is `floor(0.8 * reserve_tokens)`; prefix uses
`floor(0.5 * reserve_tokens)`, without clamping here.

`compact` consumes a borrowed preparation and returns `CompactionResult<CompactionDetails>`
from [the entry owner](entry.md). For a split turn with a nonempty prefix, history and
prefix requests start before either is awaited; an empty history uses `No prior history.`
without a history request. Prefix instructions omit prior-summary and custom focus.
The merged summary receives sorted file sections and present file details. The result
retains the prepared ID and token count; saving it is a separate caller operation.

Projection failures return JSON diagnostics before that request starts. Adapter setup
errors retain their diagnostics. Error responses use `Summarization failed: ...` or
`Turn prefix summarization failed: ...`, with `Unknown error` for absent/empty messages.
Other outcomes, including abort, return available text blocks joined with newlines.
A failed split observation does not cancel a producer-owned sibling. After generation,
an empty retained ID fails with `First kept entry has no UUID - session may need migration`.
Application compaction events, persistence and retry policy are separate operations.

This controlled adapter makes no network request:

```rust
use maestro_models::{ApiProvider, AssistantMessage, AssistantMessageEventStream,
    Model, StopReason};
use maestro_session::{SummaryOptions, generate_summary};
use serde_json::json;
use std::sync::{Arc, RwLock};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
struct ControlledSummary;
impl<E: maestro_models::ProviderExecution, O>
    maestro_models::Invoke<E, maestro_models::ProviderCall<E, O>> for ControlledSummary
{
    type Return = Result<AssistantMessageEventStream, maestro_models::DiagnosticErrorInfo>;
    fn invoke(&self, call: maestro_models::ProviderCall<E, O>) -> Self::Return {
        let model = call.model;

        let response = AssistantMessage {
            content: serde_json::from_value(json!([{"type":"text","text":"## Goal\nContinue work"}])).unwrap(),
            api: model.api, provider: model.provider, model: model.id,
            response_model: None, response_id: None, diagnostics: None,
            usage: serde_json::from_value(json!({"input":0,"output":0,"cacheRead":0,
                "cacheWrite":0,"totalTokens":0,"cost":{"input":0,"output":0,
                "cacheRead":0,"cacheWrite":0,"total":0}})).unwrap(),
            stop_reason: StopReason::Stop,
            error_message: None, timestamp: maestro_models::timestamp_now(),
        };
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(Arc::new(RwLock::new(response))));
        Ok(stream)
    }
}
let api = "controlled-summary-example";
maestro_models::register_api_provider(ApiProvider {
    api: api.into(),
    stream: Arc::new(ControlledSummary),
    stream_simple: Arc::new(ControlledSummary),
}, Some(api.into()));
let model: Model = serde_json::from_value(json!({
    "id":"controlled", "name":"Controlled", "api":api, "provider":"controlled",
    "baseUrl":"https://invalid.example", "reasoning":false, "input":["text"],
    "cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0},
    "contextWindow":200000, "maxTokens":8192
}))?;
let options = SummaryOptions {
    api_key: String::new(), headers: None, signal: None,
    custom_instructions: None, thinking_level: None,
};
let summary = generate_summary(&[json!({"role":"user","content":"Continue work"})],
    &model, 2000.0, None, &options).await?;
maestro_models::unregister_api_providers(api);
assert_eq!(summary, "## Goal\nContinue work");
# Ok(())
# }
# tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(example()).unwrap();
```
