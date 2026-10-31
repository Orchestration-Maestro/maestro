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
`prompt`, `wait_for_idle` and `abort` return only after the settle step, and so
do the idle waits of the extension and interactive command contexts. A message
sent with `send_custom_message` while a run is active is queued: steering unless
follow-up is asked for. A response that was aborted leaves queued messages
queued.

## Names

`AgentSession::set_session_name` stores the trimmed name before publishing
`session_info_changed`. The event reports the stored value. Clearing the name
publishes an event without a name. Trimming includes U+FEFF but not U+0085.

## Finalized messages

A `message_end` extension can return a replacement assistant message. Its
finalized usage cost reaches retained messages, session history, subscribers,
and the later `turn_end` and `agent_end` events. Replacing only the cost keeps
the user message and assistant content unchanged.
