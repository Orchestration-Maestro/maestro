# Retry settlement and delivered events

A prompt returns after the whole agent loop of its run: the response, any retry
that replaces it, the tools the recovered response calls and the turn that
follows their results. A later prompt is then admitted, because nothing streams
and no retry stays in flight.

The retry policy itself is in [recovery](recovery.md). A retry's backoff, and the
continuation that follows it, are part of the run: the run stays active while the
session waits, so a prompt, an idle wait and `abort` all return only after the
retry settles. A slow registered handler for a finished assistant message keeps
the prompt waiting too, because the run processes the queued events before it
decides on a retry. See [the run](lifecycle.md#the-run).

## Event order

A plain prompt delivers `agent_start`, `turn_start`, the user message's start and
end, the assistant message's start, its stream updates and its end, then
`turn_end` and `agent_end`. A turn that calls a tool inserts, between the
assistant message's end and the next turn, the tool execution's start and end,
the tool-result message's start and end, and one `turn_end`.

Adjacent stream updates arrive as one group per response. A response that runs to
completion carries the start, delta and end of each content block it streams, so
thinking, text and a tool call each stream in turn. A cancelled response stops
where it was cancelled and can omit the end of the block it was streaming.

Extension handlers hear a message's start and end before the session's own
listeners hear them, for both the user message and the assistant response.

## Terminal states

An error response and an aborted run both end with `agent_end`. An aborted run
keeps the partial assistant message it streamed: it stays the last live message
and the last stored entry, stopped as aborted.

A lost connection is an application retry like any other transient failure: the
retry start carries the provider's own error text. A retry that succeeds keeps
the recovery text; a retry that keeps failing ends with the error of its last
attempt once the retry budget is spent. No transport setup retry is involved.

## Failures that are retried

Besides overload, rate limits, 5xx statuses and lost connections, the session
retries a response whose error text says the stream ended before `message_stop`
or that the upstream request buffer limit was exceeded while retrying. A Mistral
response that finishes with the reason `error` reports "Provider stopped with:
error (server error)", so it is retried as a server failure.

A context overflow is never retried; compaction recovers it instead. The
overflow classifier recognizes the Together wording "The input (N tokens) is
longer than the model's context length (M tokens)" and the LiteLLM wording
"Requested token count exceeds the model's maximum context length of M tokens"
beside the other provider wordings.
