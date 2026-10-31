# Conversation turns

## Observable raw streams

The root operations use thread-safe callables on native targets and `Rc` callables
on browser targets. Their explicit local counterparts
have the same names under `maestro_agent::agent_loop::local`, with
`LocalAgentContext` and `LocalAgentLoopConfig`. Submitted local operations take
`AgentLoopOptionsRecord<Local>` plus an `AgentSpawner<Local>`; provider tasks reuse
that facility. Both namespaces keep one explicit custom-message type argument
and share the same operation bodies.

`agent_loop(prompts, context, config, options, spawn)` takes a shared config
(`Arc<AgentLoopConfig>` natively, `Rc<AgentLoopConfig>` in browsers) and submits one
owned prompt operation through the caller's `AgentSpawner` and returns an
`EventStream<AgentEvent, Vec<AgentMessage>>` immediately.
`agent_loop_continue(context, config, options, spawn)` returns that stream in a
`Result`: empty history yields `Cannot continue: no messages in context`, and an
assistant tail yields `Cannot continue from message role: assistant`, before
submission. Custom tails are not converted for admission.

Observing, pausing or dropping the stream does not delay or cancel submitted work.
Events are queued without reader acknowledgements. `AgentEnd` publishes its
ordered messages as the result before the producer's successful `end` call;
repeated result observations retain the message entry handles.
The wrappers share execution with the awaited operations below, using the supplied
configuration and invocation overrides. Prompt history is copied at submission,
retaining its entry handles; admitted continuations keep the supplied history live.
On producer rejection, native execution attempts to write the
original diagnostic message plus a newline to stderr and leaves the stream
unsettled without manufacturing a terminal event. Browser-host diagnostic
visibility is not yet provided; the rejected stream likewise remains unsettled.
The caller owns execution, including driving submitted tasks to completion.

## Awaited operations

`run_agent_loop` runs prompts over a copy of the supplied outer history, retaining
its entry handles. It returns the prompts and generated entries.
`run_agent_loop_continue` appends through the supplied history handle and returns
only generated entries. Empty history and an assistant tail are rejected before
observations or request conversion; custom tails are converted by the caller.

Local awaited operations take `agent_loop::LocalAgentLoopOptions`:
`RegisteredDispatch { signal, spawn }` requires the caller's provider task facility;
`ReplacementStream { signal, stream_fn }` requires no provider submission.
Threaded awaited options and function arity are unchanged. Configuration payload
and response hooks follow the selected execution family.

Each lifecycle observation is awaited. Context transformation precedes conversion;
the credential resolver runs for each request. When `AgentLoopConfig::thinking_budgets`
is present, the request snapshots that retained handle after credential resolution;
when absent, it keeps `options.thinking_budgets`. A missing or empty resolved key
uses the configured key. Invocation cancellation replaces configured cancellation,
including when absent. A replacement stream receives simple settings plus open
settings and typed objects; default dispatch uses the registered simple adapter.
See [model streams](records.md#stream-ownership) for the model-owned stream contracts.

Tools default to parallel execution. Explicit sequential configuration or a
first-matching sequential tool declaration selects sequential execution for the
batch. The first exactly named declaration is selected. Optional argument
preparation precedes [argument validation](models/arguments.md). The callback
takes `&mut Value`; object mutations stay in the retained assistant call even
if preparation or validation fails. Its returned value supplies validation
without replacing the retained call. Before and after hooks and progress events
observe the captured call's in-place object normalization, not the returned
validation arguments. The before hook may edit validated arguments without
another schema check. A blocked call
uses its nonempty reason exactly, or `Tool execution was blocked` when the reason
is absent or empty. Preparation, validation and before-hook failures skip execution
and the after hook. Execution failures reach the after hook as tool-error
results once admitted progress has settled; a progress failure rejects the
operation first.
After hooks replace supplied content and nonnull details wholesale; absent or
null override details keep the result's own details, whether absent, null or a
value. They may replace
error and termination flags, including false. An after-hook failure becomes a
tool-error result; request callback, observation, steering, follow-up and
stop-callback failures reject the operation. In a parallel batch, sibling calls
settle before that rejection returns.

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

When a failure rejects a run that `Agent::prompt` or `Agent::continue` admitted, the
agent records it as an assistant message with an error (or aborted) stop reason and
an empty text block. Listeners see that message's start and end, a turn end with no
tool results, and then the run's end, each awaited in order; the history stores the
message once, when its end is reduced.

An abort stops a tool batch where it happened. After each call, a sequential batch and a
parallel preflight check the run's signal and prepare no later sibling. The check also runs
right after the before hook, so a call whose hook aborted, or whose signal was aborted while
the hook ran, is not executed: it ends as an error result that reads `Operation aborted`.
