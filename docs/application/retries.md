# Retry settlement and delivered events

A prompt returns after the whole agent loop of its run: the response, any retry
that replaces it, the tools the recovered response calls and the turn that
follows their results. A later prompt is then admitted, because nothing streams
and no retry stays in flight.

The retry policy itself is in [recovery](recovery.md). The session's agent
listener opens the retry observation as the run ends, before the run's events
reach the queued handler that tells an extension about a finished assistant
message. A slow registered handler keeps the prompt waiting, but the observation
opens first: while the handler waits, the retry is already in flight and the
prompt is unfinished. The prompt settles when the recovered run ends.

## Event order

A plain prompt delivers `agent_start`, `turn_start`, the user message's start and
end, the assistant message's start, its stream updates and its end, then
`turn_end` and `agent_end`. A turn that calls a tool inserts, between the
assistant message's end and the next turn, the tool execution's start and end,
the tool-result message's start and end, and one `turn_end`.

Adjacent stream updates arrive as one group per response. The group carries the
start, delta and end of every content block the response streams, so a response
with thinking, text and a tool call streams each of them in turn.

Extension handlers hear a message's start and end before the session's own
listeners hear them, for both the user message and the assistant response.

## Terminal states

An error response and an aborted run both end with `agent_end`. An aborted run
keeps the partial assistant message it streamed: it stays the last live message
and the last stored entry, stopped as aborted.

A lost connection is an application retry like any other transient failure: the
retry start carries the provider's own error text, one retry then succeeds, and
the recovery text is retained. No transport setup retry is involved.
