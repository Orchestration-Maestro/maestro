# Compaction and retries

A long conversation and an unreliable provider both need the session to recover
without losing what the user asked for. Two state machines do that: compaction
shrinks the context, and retries repeat a failed response. Both run over the
owners the session was built with and announce every step to the session's
listeners.

## Compaction

The session compacts on demand, or by itself when the context grows past the
reserve. The saved preferences decide whether it happens automatically, how many
tokens stay free (`reserveTokens`, 16384 by default) and how much recent
conversation survives (`keepRecentTokens`, 20000 by default).

Calling `compact(custom_instructions)` disconnects session event forwarding,
stops a waiting retry and requests cancellation of the running turn before the
returned future is polled. It waits for the agent to settle, then records the
running compaction and announces `compaction_start` with reason `manual`. It takes the model's credentials, which
fails with the usual missing-key message, prepares the branch and stores the
result. A branch whose newest entry is already a compaction fails with `Already
compacted`; one with no cut point fails with `Nothing to compact (session too
small)`. Extensions registered for `session_before_compact` see the caller's
instructions and may refuse the compaction or supply the summary. Otherwise the
summarizer runs with the current model and thinking level over the preparation
as the handlers left it, so a handler that edits the preparation changes what
the summarizer reads. The compaction is appended, the agent history is rebuilt
from the session context, and the entry just stored reaches `session_compact`
before the result returns.

An extension context starts compaction through `CompactOptions`. If compaction
fails, or its `on_complete` callback returns an error, the session passes the
error to `on_error` when supplied. Completion callback errors keep their original
error value.

Every outcome announces `compaction_end`, and the compaction still counts as
running while its listeners run. A refusal reports its message after `Compaction
failed: `, and a cancelled compaction reports `aborted` with no message.
Aborting is `abort_compaction()`, which cancels a manual and an automatic
compaction in progress, and `abort_branch_summary()`, which cancels a branch
summary. `is_compacting()` reports any of the three.

Automatic compaction checks the newest response when a run ends and before a
prompt is sent. With the preference off, it does nothing. It skips an aborted
response unless the check runs before a prompt, and it skips a response recorded
at or before the newest compaction. When the response reports an overflow from the
selected model, the failed response leaves the agent history but stays in the
stored history, the session compacts once and asks the agent to continue 100 ms
later; a second overflow in the same turn adds no second start event and reports
that the recovery failed. When the reported usage passes the reserve instead,
the session compacts without retrying and resumes any queued messages 100 ms
later. A started user message and a successful response re-arm the one recovery
a turn gets.

Automatic compaction reads its preferences once, before `compaction_start`, so a
listener that changes them affects only the next compaction. Missing
credentials, a branch that cannot be cut and an empty history end silently with
no result. A refused compaction reports `aborted`; any other failure, whether
the summarizer, storing the entry or announcing it, reports `Context overflow
recovery failed: <message>` or `Auto-compaction failed: <message>`. After an
overflow recovery, the rebuilt history loses its newest message only when that
is a failed response. The automatic controller is recorded after the start event
while the manual one is recorded before it, so a listener that aborts during
`compaction_start` stops a manual compaction and leaves an automatic one
running.

## Retries

When a run ends, its newest response is retried when it failed with a non-empty
message matching the transient-error classifier, unless the provider reported a
context overflow, which compaction recovers. The retry is checked before
compaction.

The retry counts the attempt and refuses it once the count passes `maxRetries`
(3 by default), reporting `auto_retry_end` with
success `false`, the last attempt, the failure text and a cleared counter. A
passed attempt announces `auto_retry_start` with the attempt, the limit, the
error text and the doubling delay `baseDelayMs` (2000 by default) scaled by
`2^(attempt-1)`, removes the failed response from the agent history, waits that
long under its own cancellation and then asks the agent to continue without
delay. Aborting during the wait announces `Retry cancelled` and clears the
counter. `abort_retry()` stops a waiting retry; `abort()` stops the retry, the
turn and waits for the agent to settle.

A prompt returns after its run and, when a retry is in flight, after the retry
and the agent settle. A successful response after retries announces
`auto_retry_end` with success `true`; its listeners still read the completed
attempt from `retry_attempt()`, which returns to 0 afterwards. A final non-retryable
error after retries announces the same event with success `false`, the current
attempt and the final error, then resets the counter before checking compaction.
Exhausting the retry limit still emits only one failure event; a first non-retryable
error emits none. Dropping the
prompt never aborts the retry. `is_retrying()` reports a retry in flight and
`retry_attempt()` the attempts it has made. `auto_retry_enabled()`,
`set_auto_retry_enabled`, `auto_compaction_enabled()` and
`set_auto_compaction_enabled` read and write the saved preferences.

[Retry settlement and delivered events](retries.md) follows a prompt through the
retry, the tool loop and the event order its listeners observe.

## Events

Both state machines announce through the session's listeners, in subscription
order: `compaction_start` and `compaction_end` carry the reason (`manual`,
`threshold` or `overflow`), whether the compaction was aborted, whether the
failed prompt is retried, the failure message and the stored result;
`auto_retry_start` and `auto_retry_end` carry the attempt, the limit, the delay,
the error text, whether a retry succeeded and the final failure.

## State discipline

No borrow of the session state, the history or an agent lock is held across a
wait or a listener call, so a handler that aborts, toggles a preference or
subscribes is safe.

A retry wait, a compaction and a tree navigation run as the session's own work
on its task facility, so these methods take the session behind an `Rc`.
The returned future only watches the result. Dropping it leaves the work
running to its normal end, even when the manual compaction result was never polled: it still announces its end event and clears its
running flag. Only `abort()`, `abort_retry()`, `abort_compaction()` and
`abort_branch_summary()` stop the work. The task facility is the agent's own.
An agent supplied without one gets `tokio::task::spawn_local`, and the session's
work then runs on the caller's Tokio `LocalSet`.
