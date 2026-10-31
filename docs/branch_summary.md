# Abandoned-branch summaries

The session library collects abandoned history, prepares its newest content under a
budget and starts a summary request. It does not navigate, publish a summary, invoke
extensions or save history. These operations are available on native and browser
targets; native session-file loading remains with the [session manager](sessions/session_manager.md).
See the [compaction and branch guide](sessions/compaction.md) for application integration.

## Collection and preparation

`collect_entries_for_branch_summary(&session, old_leaf_id, target_id)` returns
borrowed chronological entries after the deepest shared ancestor, excluding that
ancestor. No old position produces no entries. Missing links stop traversal;
cycles use the session manager's bounded ancestry. Compaction entries do not stop
collection. The result owns its optional `common_ancestor_id`, not its entries.

`prepare_branch_entries(&entries, token_budget)` returns owned raw message snapshots,
`file_ops` and `total_tokens`. It scans all built-in branch-summary file details first,
then visits content newest to oldest. Tool results and metadata are excluded. Each
visited assistant's file operations are extracted before its budget check. Omitted,
zero and negative budgets are unlimited. At the first crossing, a compaction or branch
summary is included only if the accumulated estimate is strictly below 90% of budget;
traversal stops either way. Returned messages are chronological. File sets preserve
insertion order; generation computes the sorted, disjoint output lists.

Preparation reads only consumed raw fields. Consumed fields requiring valid shapes
return native JSON errors for invalid shapes; unread metadata and older messages
beyond the crossing are not decoded. Timestamp projection follows the
[session-context projection](sessions/session_manager.md#branch-selection), including invalid
timestamps becoming null.
The shared [estimator and serializer](compaction.md) own message accounting and projection.

## Generation

`generate_branch_summary(&entries, &options)` finishes preparation/projection and starts
the registered completion before returning an owned future. Caller operands can be
dropped immediately. `GenerateBranchSummaryOptions` supplies the model, explicit API
key (including empty), optional ordered headers, shared cancellation, optional custom
instructions, replacement flag and optional reserve. No reasoning or retry is added.

`generate_branch_summary_with(&entries, &options, complete)` does the same with a caller's
completion: `complete` receives the `BranchSummaryRequest` (model, context, key, headers and
signal; `simple_options` gives the invocation options) before the function returns. A session
passes its own stream function this way, so tree navigation with a summary applies the request
timeouts, retries and attribution of the session's other requests. `generate_branch_summary`
is the same call with the registered completion, `BranchSummaryRequest::complete`.

Reserve defaults to 16384; a zero context window uses 128000. The preparation budget
is context window minus reserve, without rounding or clamping. Output `maxTokens` is
always 2048. Nonempty replacement instructions replace the default branch prompt;
otherwise nonempty instructions append `Additional focus: ...`. Whitespace is meaningful.
The serialized conversation is enclosed in `<conversation>` tags and sent with the
shared `compaction::SUMMARIZATION_SYSTEM_PROMPT`.

`BranchSummaryResult` serializes without a discriminator:

| Outcome | Object members |
| --- | --- |
| No prepared content; no invocation | `summary: "No content to summarize"` only |
| Provider abort | `aborted: true` only |
| Provider error | `error` only; absent/empty text becomes `Summarization failed` |
| Success, including empty text | `summary`, `readFiles`, `modifiedFiles` |

Successful text blocks join with newlines, receive the branch preamble and optional
file sections. Nontext blocks are omitted. Projection errors fail before invocation;
thrown adapter failures propagate through the outer `Result`. A shared signal remains
live, but an aborted signal alone does not override the provider's returned outcome.
`compaction::BranchSummaryDetails` aliases the built-in two-array details record.

## Controlled example

This completion is local and makes no network request:

```rust
use maestro_models::{ApiProvider, AssistantMessageEventStream, Cancellation, Model};
use maestro_session::{BranchSummaryResult, GenerateBranchSummaryOptions,
    generate_branch_summary};
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

        let response = serde_json::from_value(json!({
            "role":"assistant", "content":[{"type":"text","text":"Continue work"}],
            "api":model.api, "provider":model.provider, "model":model.id,
            "timestamp":0, "stopReason":"stop",
            "usage":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,
                "totalTokens":0,"cost":{"input":0,"output":0,"cacheRead":0,
                    "cacheWrite":0,"total":0}}
        })).unwrap();
        let stream = AssistantMessageEventStream::new();
        stream.end(Some(Arc::new(RwLock::new(response))));
        Ok(stream)
    }
}
let api = "controlled-branch-example";
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
let options = GenerateBranchSummaryOptions {
    model, api_key: String::new(), headers: None, signal: Cancellation::new(),
    custom_instructions: None, replace_instructions: false, reserve_tokens: None,
};
let entries = vec![json!({"type":"message",
    "message":{"role":"user","content":"Continue work"}})];
let pending = generate_branch_summary(&entries, &options);
drop(entries);
drop(options);
let result = pending.await;
maestro_models::unregister_api_providers(api);
let BranchSummaryResult::Summary { summary, read_files, modified_files } = result?
    else { panic!("expected controlled success") };
assert!(summary.ends_with("Continue work"));
assert!(read_files.is_empty() && modified_files.is_empty());
# Ok(())
# }
# tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(example()).unwrap();
```
