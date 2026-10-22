# Compaction and retries

A long conversation and an unreliable provider both need the session to recover
without losing what the user asked for. Two state machines do that: compaction
shrinks the context, and retries repeat a failed response. Both run over the
owners the session was built with, are driven by the caller's clock and
cancellation, and announce every step to the session's listeners.

## Compaction

The session compacts on demand, or by itself when the context grows past the
reserve. The saved preferences decide whether it happens automatically, how many
tokens stay free (`reserveTokens`, 16384 by default) and how much recent
conversation survives (`keepRecentTokens`, 20000 by default).

`compact(custom_instructions)` stops a waiting retry, stops the running turn and
waits for the agent to settle, then records the running compaction and announces
`compaction_start` with reason `manual`. It takes the model's credentials, which
fails with the usual missing-key message, prepares the branch and stores the
result. A branch whose newest entry is already a compaction fails with
`Already compacted`; one with no cut point fails with
`Nothing to compact (session too small)`. Extensions registered for
`session_before_compact` may refuse the compaction, supply the summary, or
change the caller's instructions. Otherwise the summarizer runs with the current
model and thinking level. The compaction is appended, the agent history is rebuilt
from the session context, and the stored entry reaches `session_compact` before
the result returns.

Every outcome announces `compaction_end` first. A refusal reports its message
after `Compaction failed: `, and a cancelled compaction reports `aborted` with no
message. Aborting is `abort_compaction()`, which cancels a manual and an automatic
compaction in progress, and `abort_branch_summary()`, which cancels a branch
summary. `is_compacting()` reports any of the three.

Automatic compaction runs on `check_compaction(response, skip_aborted_check)`.
With the preference off, it does nothing. It skips an aborted response unless the
caller is checking before a prompt, and it skips a response recorded at or before
the newest compaction. When the response reports an overflow from the selected
model, the failed response leaves the agent history but stays in the stored
history, the session compacts once and asks the agent to continue 100 ms later; a
second overflow in the same turn adds no second start event and reports that the
recovery failed. When the reported usage passes the reserve instead, the session
compacts without retrying and resumes any queued messages 100 ms later.
`clear_overflow_recovery_attempt()` re-arms the one recovery a turn gets.

Missing credentials, a branch that cannot be cut and an empty history end
silently with no result. A refused compaction reports `aborted`; a summarizer
failure reports `Context overflow recovery failed: <message>` or
`Auto-compaction failed: <message>`. The automatic controller is recorded after
the start event while the manual one is recorded before it, so a listener that
aborts during `compaction_start` stops a manual compaction and leaves an
automatic one running.

## Retries

`is_retryable_error(response)` accepts a response that failed with a non-empty
message matching the transient-error classifier, unless the provider reported a
context overflow, which compaction recovers.

`handle_retryable_error(response)` counts the attempt, and refuses it once the
count passes `maxRetries` (3 by default), reporting `auto_retry_end` with
success `false`, the last attempt, the failure text and a cleared counter. A
passed attempt announces `auto_retry_start` with the attempt, the limit, the
error text and the doubling delay `baseDelayMs` (2000 by default) scaled by
`2^(attempt-1)`, removes the failed response from the agent history, waits that
long under its own cancellation and then asks the agent to continue without
delay. Aborting during the wait announces `Retry cancelled` and clears the
counter. `abort_retry()` stops a waiting retry; `abort()` stops the retry, the
turn and waits for the agent to settle.

`wait_for_retry()` waits for the retry in flight and then for the agent to
settle; dropping the wait never aborts the retry. `is_retrying()` reports a retry
in flight and `retry_attempt()` the attempts it has made. `auto_retry_enabled()`,
`set_auto_retry_enabled`, `auto_compaction_enabled()` and
`set_auto_compaction_enabled` read and write the saved preferences.

## Events

Both state machines announce through the session's listeners, in subscription
order: `compaction_start` and `compaction_end` carry the reason (`manual`,
`threshold` or `overflow`), whether the compaction was aborted, whether the failed
prompt is retried, the failure message and the stored result; `auto_retry_start`
and `auto_retry_end` carry the attempt, the limit, the delay, the error text,
whether a retry succeeded and the final failure.

## State discipline

No borrow of the session state, the history or an agent lock is held across a
wait or a listener call, so a handler that aborts, toggles a preference or
subscribes is safe. Every wait is the caller's clock and its own cancellation
signal, so a front end can close cleanly mid-wait.
