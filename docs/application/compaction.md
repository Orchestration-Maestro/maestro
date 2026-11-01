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

### Persistence

File-backed sessions append compaction to their JSONL history. Reopening that
file restores the summary and the same retained messages. Compaction does not
replace the session: the next prompt uses the rebuilt context and saves its
response in the existing history.

### Extension events

`session_before_compact` supplies the ordered branch entries, messages to
summarize, split-turn prefix, token estimate, settings and cancellation signal.
The callback context exposes the current model, model registry and session
manager. A handler can return `cancel: true` to refuse compaction before the
summarizer runs.

`session_compact` runs after the entry is saved. Its `compaction_entry` is already
readable through the callback's session manager, and `from_extension` reports
whether an extension supplied the result. A cancelled compaction stores no entry
and sends no `session_compact` event.

Handlers run in registration order. A failed before handler reports an attributed
extension error and leaves default summarization available. An extension may
instead return a `CompactionResult`; Maestro preserves its summary, retained
entry ID, token estimate and JSON details without replacing them with prepared
values. Missing details remain absent; explicit null remains null.

### Automatic compaction

Automatic compaction starts when reported context use is strictly greater than
`contextWindow - reserveTokens`. Equality does not trigger compaction. Disabling
automatic compaction suppresses these checks without disabling manual compaction.

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
