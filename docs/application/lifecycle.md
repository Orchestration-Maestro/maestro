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

## Names

`AgentSession::set_session_name` stores the trimmed name before publishing
`session_info_changed`. The event reports the stored value. Clearing the name
publishes an event without a name. Trimming includes U+FEFF but not U+0085.

## Finalized messages

A `message_end` extension can return a replacement assistant message. Its
finalized usage cost reaches retained messages, session history, subscribers,
and the later `turn_end` and `agent_end` events. Replacing only the cost keeps
the user message and assistant content unchanged.
