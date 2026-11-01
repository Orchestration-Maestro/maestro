# Compaction

## Quick start

Call `AgentSession::compact(None)` to summarize older conversation while keeping
recent messages. Pass `Some("focus on the remaining work")` to guide the summary.
The result contains the summary, the first retained entry ID and the context token
count before compaction.

## Using compaction

### Manual compaction

Compaction stops the current turn before preparing the selected branch. It saves
a compaction entry, then rebuilds the live conversation as a summary followed by
the retained messages. The original entries remain in the session tree.

A split turn produces a history summary and a summary of the turn's early
messages. The combined summary places history first, then the turn context.
Tool results stay with their calls when Maestro chooses the retained range.

A memory-only session supports compaction without creating a session file.
Manual compaction announces one `compaction_start` and one `compaction_end`, both
with reason `manual`. A successful end carries the result with `aborted: false`
and `willRetry: false`.

## Reference

`compact(custom_instructions)` returns an error when credentials are unavailable,
the branch cannot be compacted, an operation fails or compaction is cancelled.
`abort_compaction()` cancels a running compaction. `is_compacting()` reports
whether compaction or branch summarization is running.

See [compaction and retries](recovery.md) for recovery and cancellation details.

## Examples

To focus a summary on unresolved work, call
`session.compact(Some("keep the open questions and changed files")).await`.
Use the returned `first_kept_entry_id` to identify the retained boundary rather
than guessing it from a message count.
