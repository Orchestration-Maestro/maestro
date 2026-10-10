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

Tools run sequentially in assistant content order, including repeated calls.
The first exactly named declaration is selected. Optional argument preparation
precedes [argument validation](models/arguments.md). Preparation and execution
failures become tool-error artifacts; request callback and observation failures
reject the operation at their callback boundary.

Progress futures run alongside execution and may overlap. Execution completion
closes progress admission; admitted futures settle before finalization or a
progress failure is returned. The first observed progress failure wins over an
execution failure. Execution-end is awaited before the artifact timestamp is
assigned. Artifact observations retain the entry subsequently appended to history;
turn-end tool results are owned snapshots. Batch results enter history after every
call finishes. Another assistant turn runs unless the nonempty batch's results
all request termination. An error or aborted assistant bypasses tool execution.

This operation currently executes plain sequential turns. Hooks, steering and
follow-up polling, stop-after-turn and parallel scheduling are delivered separately
in [the control delivery](https://github.com/Orchestration-Maestro/maestro/issues/497).
Stored tool scheduling preferences do not change this operation yet.
