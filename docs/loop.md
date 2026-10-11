# Awaited conversation turns

`run_agent_loop` runs prompts over a copy of the supplied outer history, retaining
its entry handles. It returns the prompts and generated entries.
`run_agent_loop_continue` appends through the supplied history handle and returns
only generated entries. Empty history and an assistant tail are rejected before
observations or request conversion; custom tails are converted by the caller.

Each lifecycle observation is awaited. Context transformation precedes conversion;
the credential resolver runs for each request. A missing or empty resolved key
uses the configured key. Invocation cancellation replaces configured cancellation,
including when absent. A replacement stream receives simple settings plus open
settings and typed objects; default dispatch uses the registered simple adapter.
See [model streams](records.md#stream-ownership) for the model-owned stream contracts.

Tools default to parallel execution. Explicit sequential configuration or a
first-matching sequential tool declaration selects sequential execution for the
batch. The first exactly named declaration is selected. Optional argument
preparation precedes [argument validation](models/arguments.md), then the before
hook may edit validated arguments without another schema check. A blocked call
uses its nonempty reason exactly, or `Tool execution was blocked` when the reason
is absent or empty. Preparation, validation and before-hook failures skip execution
and the after hook. Execution failures reach the after hook as tool-error
results once admitted progress has settled; a progress failure rejects the
operation first.
After hooks replace supplied content and nonnull details wholesale and may replace
error and termination flags, including false. An after-hook failure becomes a
tool-error result; request callback and observation failures reject the
operation. In a parallel batch, sibling calls settle before that rejection
returns.

Progress futures run alongside execution and may overlap. A callback reserves
admission before invoking the synchronous observer outside locks. Execution
completion closes admission to new callbacks; reserved observations finish their
synchronous calls and settle their returned futures before finalization or a
progress failure is returned. A retained callback invoked after admission closes
does not run the observer. The first observed progress failure wins over an
execution failure. Execution-end is awaited before the artifact timestamp is
assigned. Artifact observations retain the entry subsequently appended to history;
turn-end tool results are owned snapshots. Batch results enter history after the
batch artifact observations. Sequential calls finish their artifacts before the
next start. Parallel batches preflight in source order before execution begins;
immediate errors emit execution-end during preflight. All allowed execute callbacks are invoked in source order before any allowed
execution-end. Execution futures and after hooks are driven concurrently; execution
ends follow finalization completion, while artifacts retain source order, including
repeated call IDs. Artifact timestamps are assigned after the
parallel finalizers settle. If an admitted finalizer fails, siblings settle before
the first observed failure is returned; successful sibling execution ends remain,
but no batch artifacts, turn-end or agent-end are emitted.

Initial steering is polled after entry observations. After a successful assistant
turn, the stop hook follows the awaited turn-end observation and sees appended
results. A true stop skips subsequent queue polls. Otherwise steering is polled
before follow-ups; follow-ups are polled only when tools do not require continuation
and steering is empty. Queued entries receive awaited message-start/end pairs
before insertion, preserving their order and handles. A nonempty tool batch
continues unless its final results unanimously request termination; steering and
follow-ups may still resume afterward. Error or aborted assistants bypass tools
and post-turn controls, not the initial steering poll.
