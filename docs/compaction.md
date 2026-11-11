# Compaction

`maestro_session::summarization` prepares summary input and generates summaries of
history or of a split turn. It never mutates history and never persists entries.
[Abandoned-branch generation](branch_summary.md) is a separate interface with its
own budget and result policy. The [complete guide](sessions/compaction.md) covers
retained spans, navigation, formats, settings and caller-owned extension
integration. The operations are also re-exported at the crate root and work on
native and browser targets. Session traversal and storage stay with the
[session manager](sessions/session_manager.md).

## Prepare a branch

Supply raw `SessionEntry` values in branch order, with usable IDs and parent links.
`prepare_compaction` returns owned history messages and split-turn prefix messages,
the first retained ID, tokens-before, the previous summary, file operations and
settings. It returns `None` when the branch ends in a compaction or has no usable
first-kept ID. When a selected field it reaches is invalid, it returns
`serde_json::Error`; unrelated raw fields stay intact. An empty previous summary
stays present. The settings' automatic-compaction trigger does not gate manual
preparation.

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
20000 recent tokens. `should_compact` fires only when usage is strictly greater
than the context window minus the reserve; disabled settings never trigger it.
`calculate_context_tokens` uses the reported total when it is nonzero (an omitted
or null total counts as absent); otherwise it adds the input, output, cache-read
and cache-write counts.

`estimate_context_tokens` takes the latest successful assistant usage, skipping
error and aborted attempts and usage that reports no tokens, and estimates the
messages after it. With no usable usage it estimates every supplied message. Usage
from an answer stamped earlier than a later message is stale, and the estimate
ignores it until a newer answer reports usage. A compaction summary inserted ahead
of the retained span is one such later message.

`estimate_tokens` rounds up UTF-16 code units divided by four. It counts assistant
text, thinking and canonical JSON tool arguments, shell command and output (even
when excluded from model context), and unwrapped summary text. Each image in a
user, custom or tool-result message adds 4800 units. Unknown roles contribute zero.

`find_cut_point` takes an existing half-open index range. It walks backwards and
counts every message an entry puts in context, including custom messages, nonempty
branch summaries and compaction summaries. It crosses at the retained budget and
selects the next valid cut, falling back to the first candidate. Tool results
cannot start a cut. Preceding metadata is absorbed, but never across an entry that
is in context or across a compaction boundary.

`find_turn_start_index` treats these as turn starts: user, shell, custom,
branch-summary and compaction-summary messages, and the custom-message and
branch-summary entries that become them. Stored compaction entries never start a
turn or a cut. A cut that is not itself a turn start but has a turn start before it
produces a split prefix, which may be empty.

## Shared summary input

`create_file_ops` and `extract_file_ops_from_message` track nonempty string paths
on assistant read, write and edit calls. Preparation tracks history and split
prefixes, not the kept suffix. It carries previous built-in file details only when
`fromHook` is falsy. `compute_file_lists` removes modified paths from the read
list, unions edits and writes, and sorts both lists in UTF-16 lexical order.
`format_file_operations` keeps the supplied list order, and any duplicates, in the
read-files and modified-files sections. Preparation reads each entry's message
through the same projection as the cut search, so an empty branch summary and a
compaction summary are not summary input.

`serialize_conversation` takes typed model messages, not raw application messages.
Project application messages through the [message conversion interface](sessions/messages.md)
first. It emits user, assistant, assistant-tool-call and tool-result labels,
separated by blank lines. Assistant thinking is left out: providers return
summaries of the reasoning, and replaying it can make the summary request larger
than the context it summarizes. Tool arguments use canonical JSON key and number
spelling, and assistant categories appear in text and tool order.

Only tool-result text is clipped, at 2000 Unicode scalar values, with an
omitted-character notice. User and assistant text stay complete, and stored content
is not rewritten.

## Generate a summary

`generate_summary` takes raw application messages, a supplied model, reserve
tokens, an optional prior summary and `SummaryOptions`. It reads only
summary-visible fields, formatted as in the [message projection](sessions/messages.md)
and the shared serializer above. It starts the registered model completion before
it returns an owned future, so borrowed inputs need not outlive that future. Each
call uses a fresh request timestamp.

`generate_summary_with_usage` takes the same operands and returns a
`GeneratedSummary`: the summary text with the `Usage` the provider reported for
that request. `generate_summary` still returns the text alone.

Supply the API key explicitly (an empty key is kept), optional ordered headers and
shared cancellation. Reasoning is omitted unless the model supports it and you
select an enabled level explicitly; no thinking level is chosen by default. A
nonempty prior summary selects update instructions, and nonempty custom
instructions add a history focus. Whitespace counts as nonempty. The history
budget is `floor(0.8 * reserve_tokens)` and the prefix budget is
`floor(0.5 * reserve_tokens)`, with no clamping here.

`compact` consumes a borrowed preparation and returns
`CompactionResult<CompactionDetails>` from [the entry owner](entry.md). For a split
turn with a nonempty prefix, the history and prefix requests both start before
either is awaited. An empty history uses `No prior history.` and sends no history
request. Prefix instructions omit the prior summary and custom focus. The merged
summary carries sorted file sections and any present file details. The result keeps
the prepared ID and token count; saving it is a separate caller operation.

Projection failures return JSON diagnostics before the request starts. Adapter
setup errors keep their diagnostics. Error responses use `Summarization failed: ...`
or `Turn prefix summarization failed: ...`, with `Unknown error` when the message
is absent or empty. Any other outcome, abort included, returns the available text
blocks joined with newlines. A failed split observation does not cancel a sibling
the producer owns. After generation, an empty retained ID fails with
`The first retained entry has no UUID; the session file may need migrating`. Application compaction
events, persistence and retry policy are separate operations.

This controlled adapter sends no network request:

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
