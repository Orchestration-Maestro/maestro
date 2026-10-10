# Agent state and queued input

`maestro-agent` owns live conversation state, shared message and executable-tool
entries, and independent steering and follow-up input queues. Execution is
tracked in [the awaited loop](https://github.com/Orchestration-Maestro/maestro/issues/476)
and [the Agent lifecycle](https://github.com/Orchestration-Maestro/maestro/issues/477).

Tools of any parameter and detail types are retained as `SharedAgentTool`
entries and recovered with `downcast_ref`.

State collection replacement retains the supplied entry handles in new outer
storage. A retained history handle keeps its collection after the state slot is
replaced. Callers release their state and collection guards before invoking
another operation on the same state. State setters run under the caller's guard;
they do not promise destructor reentry through that guard.

Enqueue stores a message without appending it to history. Both queue policies
default to `OneAtATime` when omitted; changing either policy does not consume input. This API
stores policies but does not yet expose queue consumption.

Reset clears history and queued input while keeping instructions, model, tools
and queue modes. Agent-owned reset and clear release discarded entries outside
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
