# Session observations

See [embedding a session](sdk.md) for session construction and subscriptions,
and [session operations](session-operations.md) for history and replacement
operations.

## Names

`AgentSession::set_session_name` stores the trimmed name before publishing
`session_info_changed`. The event reports the stored value. Clearing the name
publishes an event without a name. Trimming includes U+FEFF but not U+0085.

## Finalized messages

A `message_end` extension can return a replacement assistant message. Its
finalized usage cost reaches retained messages, session history, subscribers,
and the later `turn_end` and `agent_end` events. Replacing only the cost keeps
the user message and assistant content unchanged.
