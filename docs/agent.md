# Agent state and owned prompts

`maestro-agent` owns live conversation state, shared message and executable-tool
entries, and independent steering and follow-up input queues.
The [awaited conversation operations](loop.md) accept turn controls and tool-batch
execution preferences.
`Agent::prompt` synchronously reserves a run and submits its work through
`AgentOptions::spawn`. The caller supplies a facility that retains and drives the
submitted future independently of the returned completion observer. Construction
is inert; prompting without that facility returns
`Agent requires a task spawner before prompting.`

Dropping prompt observation does not cancel submitted work. `abort` only signals
cooperative cancellation, and `wait_for_idle` observes completion without returning
history. `stream_fn` replaces model dispatch for subsequently admitted runs.
Replacement streams receive the default parallel tool-execution preference in open options.
Run admission copies outer history/tool collections while retaining entry handles
and the selected model handle. Replacing the live model slot does not retarget
admitted work; synthetic failure records use the current live model.

`subscribe` deduplicates callback identity. Dispatch reduces state before awaiting
live listeners in registration order; additions and removals can affect an ongoing
walk. Listeners share a mutable event handle. `Subscription::unsubscribe` removes
the callback identity explicitly; dropping the subscription does not remove it.
Caller-created strong captures of the Agent require explicit deregistration.

Continuation, queue consumption and remaining request configuration are tracked in
[the lifecycle follow-up](https://github.com/Orchestration-Maestro/maestro/issues/541).
Delegated request/tool behavior is documented in [conversation operations](loop.md).

`SharedAgentTool` entries expose declarations, labels, callbacks and scheduling
preferences directly, including mutable access through the shared handle.
`AgentTool::typed` adapts a typed callback to JSON arguments and details;
argument decoding failures return without invoking its body. Typed results can
be read with `serde_json::from_value`. Preparation remains JSON-to-JSON.
Arguments are decoded with [`serde_json::from_value`](https://docs.rs/serde_json/latest/serde_json/fn.from_value.html) and details encoded with
[`serde_json::to_value`](https://docs.rs/serde_json/latest/serde_json/fn.to_value.html); their documentation owns the conversion rules.
Serialization failures return through the tool error channel.

State collection replacement retains the supplied entry handles in new outer
storage. A retained history handle keeps its collection after the state slot is
replaced. Callers release their state and collection guards before invoking
another operation on the same state. State setters run under the caller's guard;
they do not promise destructor reentry through that guard.

Enqueue stores a message without appending it to history. Both queue policies
default to `OneAtATime` when omitted; changing either policy does not consume input. This API
stores policies but does not yet expose queue consumption.

Reset clears history and queued input while keeping instructions, model, tools
and queue modes without aborting active work or releasing run admission. Agent-owned reset and clear release discarded entries outside
internally acquired guards. Poisoned standard-library locks retain their value.

```rust
use maestro_agent::{Agent, AgentOptions, AgentInitialState, QueueMode};

let agent: Agent = Agent::new(AgentOptions {
    initial_state: AgentInitialState {
        system_prompt: Some("Answer concisely.".into()),
        ..Default::default()
    },
    steering_mode: Some(QueueMode::All),
    ..Default::default()
});
let retained = agent.state().clone();
agent.reset();
assert_eq!(retained.read().unwrap().system_prompt, "Answer concisely.");
```

Message descriptors, tool declarations and content records use the existing
[model records](records.md). The custom-message contract keeps caller-defined
roles and payloads typed; it does not convert them to JSON. Tool callbacks are
retained rather than invoked by state and queue operations.
