# Streamed agent conversations

`maestro-agent` owns text-conversation execution. Inject an `ApiStreamSimpleFunction` and an
explicit `Model`. The supplied callable owns model invocation. The agent does not execute tools,
retry requests or persist sessions.

## Prompt and continuation

`prompt` admits one timestamped `AgentMessage`. `continue_run` uses existing
history without replaying its message events. Empty history returns
`NoUsableHistory`, even with queued input. An assistant tail returns `AssistantTail`
unless queued input can restart it: steering is chosen before follow-up.
Application tails are allowed; their converter must produce usable model input.
Admission is synchronous and atomic. Overlapping calls return `Busy` without
changing history or queues. No current Tokio runtime returns `RuntimeUnavailable`
before mutation.

A runtime-owned run begins immediately. Await its `RunHandle` for new records
only, excluding prior history. Dropping a completion or idle waiter does not
cancel the run. Keep the Tokio runtime alive until settlement. A panicking
callback violates its contract; join-observed task failure returns `RunFailed`
to completion and idle waiters, without promising normal terminal events. A new
admitted run replaces the prior completion outcome.

## Messages and model options

`AgentMessage::Model` uses the shared model union. `Application` stores an open
kind string, arbitrary JSON and a supplied timestamp. Optional `transform_context`
receives a detached message view and shared cancellation before `convert_messages`.
Neither changes raw history. Default transformation is identity; default conversion
passes model records through and filters application records.

Supply `SimpleStreamOptions` per run. The agent forwards them for every turn,
with the current separate system prompt and no tool declarations. A missing
signal receives a run-local cancellation handle; supplied signals stay shared.
Authentication and provider preferences belong to the supplied callable. Authoritative model terminal records replace partials;
usage, response identity and failures are never rebuilt from deltas. Error/aborted
assistant outcomes are conversation records, not `AgentError` or retry requests.
Unexpected tool content remains data; no invented result or tool continuation runs.

## Queues and stopping

Steering and follow-up are independent FIFO queues. Enqueue does not edit history
or start execution. Both default to `QueueMode::OneAtATime`; `All` consumes the
current FIFO on one poll. `queue_mode` and `set_queue_mode` inspect/change one
policy without changing the other. `queue` returns a detached copy; `clear_queue`
returns removed records in FIFO order and leaves the other queue untouched.

Entry steering is polled after awaited agent start, turn start and initial input
start/end events, before the first model request. It is also polled after a
completed turn. Only an assistant-tail restart supplied by steering skips the
entry poll, preserving one-at-a-time semantics; a follow-up restart still polls
entry steering. Follow-up is consumed only after steering is exhausted. There is
no executable-tool continuation in text execution.

After awaited `TurnEnd` subscribers, `stop_after_turn` sees the terminal assistant,
completed context and run-local records. True stops before either queue poll;
false creates no extra request. Model error/abort stops before this hook and before
post-turn queue polling. There is no universal turn limit or execution deadline.

## Events, cancellation and idle

Prompt order is agent start, turn start, input start/end, assistant start,
cumulative updates, assistant end, turn end, agent end. Continuation does not
re-emit existing history. Subsequent turns start before queued message events.
Even a terminal error event without model start has balanced assistant start/end events.
An invocation setup error settles an error assistant through the same history and
end lifecycle. Iterator EOF awaits the independently supplied stream result,
including producers that close with `end(Some(message))` without a terminal event.
`MessageUpdate` retains both the outer cumulative snapshot and original nested
model event. State is reduced before each subscriber observes it. Returned state,
queue and event values are owned independent snapshots.

Subscribers are awaited in registration order without locks across user code.
Each event snapshots registrations; unsubscribe is idempotent and affects future
acceptance, not already accepted delivery. Removed listener cleanup runs without
the state lock. No replay or initial event occurs.
Callbacks must not panic or await their own run's completion/idle. Transforms
must settle cooperatively. A sink must await any child work it wants included in
idle; detached work is excluded.

`abort` only signals active cancellation. It does not abort the task, undo remote
effects, clear queues or abandon accepted callbacks. Idle abort is harmless.
`is_running` remains true through awaited `AgentEnd` sinks. Completion and
`wait_for_idle` wait for owned execution and those sinks. Idle observes the run
when polled, not future submissions. Low-level `AgentEnd`/idle is **not** universal
application settlement (retry, compaction and application work have other owners).

## Credential-free example

```rust
# async fn example() -> Result<(), Box<dyn std::error::Error>> {
use maestro_agent::{Agent, AgentMessage, AgentOptions};
use maestro_models::*;
use std::sync::{Arc, RwLock};
let descriptor = Model {
    id: "synthetic".into(), name: "Synthetic".into(), api: "synthetic".into(),
    provider: "local".into(), base_url: String::new(), reasoning: false,
    thinking_level_map: None, input: vec!["text".into()],
    cost: TokenRates { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0 },
    context_window: 0.0, max_tokens: 0.0, headers: None, compat: None,
};
fn produce(model: Model) -> Result<AssistantMessageEventStream, ThrownValue> {
    let message = Arc::new(RwLock::new(AssistantMessage {
        content: vec![], api: model.api, provider: model.provider, model: model.id,
        response_model: None, response_id: None, diagnostics: None,
        usage: Usage { input: 0.0, output: 0.0, cache_read: 0.0, cache_write: 0.0,
            total_tokens: 0.0, cost: UsageCost { input: 0.0, output: 0.0,
                cache_read: 0.0, cache_write: 0.0, total: 0.0 } },
        stop_reason: StopReason::Stop, error_message: None, timestamp: 0.0,
    }));
    let stream = create_assistant_message_event_stream();
    stream.push(AssistantMessageEvent::Start { partial: message.clone() })?;
    stream.push(AssistantMessageEvent::Done { reason: StopReason::Stop, message })?;
    Ok(stream)
}
let supplied: ApiStreamSimpleFunction = Arc::new(|model, _, _| produce(model));
let agent = Agent::new(supplied, descriptor, AgentOptions::default());
let input = AgentMessage::Model(Message::User(UserMessage {
    content: UserContent::Text("Hello".into()), timestamp: 17.0,
}));
let new_messages = agent.prompt(input, SimpleStreamOptions::default())?.await?;
assert_eq!(new_messages.len(), 2);
agent.wait_for_idle().await?;
# Ok(())
# }
# let runtime = tokio::runtime::Builder::new_current_thread().build().unwrap();
# runtime.block_on(example()).unwrap();
```

During an active run, inspect queues before cooperative abort:

```rust
# use maestro_agent::{Agent, Queue};
# async fn cancel(agent: &Agent) -> Result<(), maestro_agent::AgentError> {
let pending_steering = agent.queue(Queue::Steering);
agent.abort();
agent.wait_for_idle().await?;
assert_eq!(agent.queue(Queue::Steering), pending_steering);
# Ok(())
# }
```

The last assertion assumes no concurrent enqueue/clear and abort before the next
queue poll; queue inspection is not a submission fence.
