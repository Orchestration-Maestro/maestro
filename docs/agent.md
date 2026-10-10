# Agent state and queued input

`maestro-agent` owns live conversation state, shared message and executable-tool
entries, and independent steering and follow-up input queues. Execution is
tracked in [the awaited loop](https://github.com/Orchestration-Maestro/maestro/issues/476)
and [the Agent lifecycle](https://github.com/Orchestration-Maestro/maestro/issues/477).

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
