# Session observations

See [embedding a session](sdk.md) for session construction and subscriptions,
and [session operations](session-operations.md) for history and replacement
operations.

## Replacement lifetime

New, resumed and forked sessions can be vetoed before replacement. Shutdown
handlers finish before the host's invalidation callback; the old contexts then
become stale. A failing invalidation callback stops the replacement before the
old session is disposed. If configured, the host's rebind callback finishes
before the `with_session` continuation runs. Use that continuation's context for
messages to the new session, not a captured command context or extension API.
Reads from the old command frame fail recoverably after replacement.

A factory failure after teardown does not roll the old session back into service.
Its handle remains selected but its extension contexts are stale.

## The run

Every start of the agent is one run: `AgentSession::prompt`, and
`send_custom_message` with `trigger_turn`, call the same function. The run marks
the session active, prompts the agent and then, until nothing is left to do,
retries a failed response after its backoff, compacts when the context is full
and continues the agent for the queued messages. It settles last: shell results
recorded while it ran are appended, the session is marked idle and every waiter
is released.

`is_streaming` is true from the start of the run through its last continuation.
`is_idle` is its inverse: retry waits, compaction and queued continuations all belong
to the active run. A second start is refused without changing that run or its waiters.
An extension shutdown requested during a run waits for settlement; an interactive
request made while idle starts shutdown at once.
`prompt`, `wait_for_idle` and `abort` return only after the settle step, and so
do the idle waits of the extension and interactive command contexts. `dispose` stops
the work a run waits on (a retry wait, a compaction, a branch summary, a shell command and
the response) without awaiting it, and the run then settles on its own. A message
sent with `send_custom_message` while a run is active is queued: steering unless
follow-up is asked for. A response that was aborted leaves queued messages
queued.

A custom message sent with `trigger_turn` set to false while a run is active starts no turn
and steers nothing. The session keeps it until the current turn ends and records it right
after that turn's tool results, so it never lands between a tool call and its result; the
history, the session and the message events show the same order. A message still kept when
the run settles is recorded at the end of the history.

A change to the active tools, the selected model or the thinking level during a run
reaches the next request of the run; the request already sent keeps its payload. A system
prompt the extensions set for the run, even an empty one, stays in force for every turn of the
run, and the base prompt returns when the run settles.

If a retry ends with a non-retryable error, `auto_retry_end` reports failure, the
current attempt and the final error, then clears the attempt count before the
compaction check. A first error without retries emits no retry-end event.

## Reload

`AgentSession::reload` reads the settings again and gives the live agent the steering and
follow-up modes they now hold, so the next queued input uses them without a restart.
Reload emits `session_start` and extends extension resources only when a UI context,
command actions, shutdown handler or error handler is bound. An abort handler alone
does not count.

## Names

`AgentSession::set_session_name` stores the trimmed name before publishing
`session_info_changed`. The event reports the stored value. Clearing the name
publishes an event without a name. Trimming includes U+FEFF but not U+0085.

## Finalized messages

A `message_end` extension can return a replacement assistant message. Its
finalized usage cost reaches retained messages, session history, subscribers,
and the later `turn_end` and `agent_end` events. Replacing only the cost keeps
the user message and assistant content unchanged.
