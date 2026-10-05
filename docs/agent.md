# Streamed agent conversations

`maestro-agent` owns streamed conversation and executable-tool turns. Inject
`Arc<Models>` and an explicit `Model`; the shared model registry owns adapter
selection, authentication, request projection and stream normalization. The agent
does not retry requests or persist sessions.

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

Supply `StreamOptions` per run, including request authentication and cancellation.
The agent forwards them unchanged for every turn, with the current separate system
prompt and the ordered model declarations from `AgentOptions.tools`. Supported preferences are resolved by models,
not by agent defaults. Authoritative model terminal records replace partials;
usage, response identity and failures are never rebuilt from deltas. Error/aborted
assistant outcomes are conversation records, not `AgentError` or retry requests.
Completed calls from successful stop, length or tool-use outcomes execute only
after the authoritative assistant ends. Partial arguments and error/aborted
assistants never execute, even when a failed assistant retains a completed call.

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
entry steering. After a tool batch, steering precedes ordinary nonterminating
tool continuation; follow-up waits for both to finish.

After awaited `TurnEnd` subscribers, `stop_after_turn` sees the terminal assistant,
completed source-ordered tool results, context and run-local records. True stops
before either queue poll or ordinary tool continuation; false creates no extra
request for a text-only turn. Model error/abort stops before this hook and before
post-turn queue polling. There is no universal turn limit or execution deadline.

## Events, cancellation and idle

Prompt order is agent start, turn start, input start/end, assistant start,
cumulative updates, assistant end, turn end, agent end. Continuation does not
re-emit existing history. Subsequent turns start before queued message events.
Even a setup failure without model start has balanced assistant start/end events.
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

## Executable tools

Supply ordered `Tool` values in `AgentOptions.tools` (default empty). Only their
existing `ToolDeclaration` values reach the model. Lookup uses the first exact,
case-sensitive name match without trimming or normalization. A tool adds a label,
optional asynchronous preparation callback, executor and optional scheduling mode.
No tool output schema or usage record is introduced.

The lifecycle is resolve → prepare current-format object input → shared schema
validation/coercion → registered before hooks → execute → drain accepted progress
→ registered after hooks → execution end → tool-result message. Preparation and
all hooks receive the same cooperative run cancellation. `ToolInvocation` carries
the call ID, final working arguments, cancellation and synchronous progress sink.
Do not retain/use the progress sink after the execution future settles. Submission
immediately owns output and the subscription snapshot; asynchronous delivery is
awaited in submission order before after hooks, even on execution failure.

Before hooks see previous whole-object replacements, without revalidation.
Blocking wins over an accompanying replacement and skips remaining before hooks,
execution and all after hooks. Hook contexts are detached assistant-completed
batch snapshots, excluding results from this batch, even in sequential mode.
Raw assistant calls and start/update arguments never change.

After hooks run on executed success or failure and see accumulated output. Each
supplied content, details, error or termination field replaces the whole field;
omission retains it. Empty content, JSON null details and false flags are real
replacements, not omissions. Nested details are replaced, not merged. A low-level
after-hook failure stops that chain and replaces the accumulated output with an
error; handler diagnosis/preservation belongs to a host adapter.

Expected callback failures use their diagnostic string verbatim as one text block,
`{}` details, `is_error = true` and no termination hint. Unknown tools report
`Tool {name} not found`; validation uses the shared safe validation diagnostic.
An absent or empty block reason uses `Tool execution was blocked`, but whitespace
reasons survive verbatim. Output text does not itself imply an error.

`ToolResult` carries text/images, arbitrary JSON details and an optional runtime-only
termination hint. Finalized content and the separate error flag reach the next
request; details stay in raw history/hooks/events and the existing model projection
removes them. `ToolResultMessage` never stores termination. The optional
`AgentOptions.clock` supplies Unix milliseconds sampled once at result-message
creation, after awaited execution end. Absent a clock, system time is used; no
assistant timestamp is reused.

Parallel is the default. Preflight completes serially in source order before any
allowed executor launches. Immediate rejections end during preflight. Executed
ends follow finalized completion order; result messages wait for all ends and retain
source positions, not name, ID or timestamp order. Global sequential or any resolved
tool's sequential override serializes the entire batch, even when that tool fails
preflight; each complete call and result delivery precedes the next start. There is
no execution cap or deadline.

A nonempty batch suppresses ordinary continuation only when every finalized output
has `terminate == Some(true)`. Mixed false, omitted hints or errors continue.
Queued steering and follow-up may still restart after a terminating batch, unless
stop-after-turn returns true. Priority is awaited turn end → stop hook → steering
→ ordinary tool continuation → follow-up.

`AgentState.pending_tool_calls` is a detached insertion-ordered set view. Start adds
and end removes an ID before subscribers run; settlement clears it. It is observation,
not execution authorization. Abort signals preparation, hooks and tools without
abandoning launched calls or accepted progress. Completion/idle await settlement;
tools that refuse to cooperate can keep a run active indefinitely. Callbacks,
clocks, notification/wake and captured-value cleanup run outside state locks.

## Credential-free example

```rust
use maestro_agent::{Agent, AgentMessage, AgentOptions, Tool, ToolResult};
use maestro_models::{InputContent, Message, Model, ModelIdentity, Models,
    ProviderUpdate, RequestAuth, Script, ScriptStep, ScriptedProvider,
    StopReason, StreamOptions, TextContent, ToolDeclaration, UserMessage};
use std::sync::Arc;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let model = Model {
    identity: ModelIdentity {
        provider: "example".into(), model: "text".into(), operation: "chat".into(),
    },
    name: "Synthetic text".into(), endpoint: "synthetic:endpoint".into(),
    chat: Some(maestro_models::ChatMetadata { context_window: None }),
    protocol: "synthetic".into(), rates: None, capabilities: Default::default(),
    headers: Default::default(), input: vec!["text".into()],
};
let provider = Arc::new(ScriptedProvider::new(vec![Script::Steps(vec![
    ScriptStep::Update(ProviderUpdate::ToolCallStart {
        content_index: 0, id: "call".into(), name: "echo".into(),
        replay_metadata: None,
    }),
    ScriptStep::Update(ProviderUpdate::ToolCallDelta {
        content_index: 0, delta: r#"{"text":"Hello"}"#.into(),
    }),
    ScriptStep::Update(ProviderUpdate::ToolCallEnd { content_index: 0 }),
    ScriptStep::Update(ProviderUpdate::Done { reason: StopReason::ToolUse }),
])]));
let mut models = Models::new(Arc::new(|| 42));
models.register(model.clone(), provider)?;
let echo = Tool {
    declaration: ToolDeclaration {
        name: "echo".into(), description: "Echo supplied text".into(),
        parameters: serde_json::json!({
            "type": "object", "properties": {"text": {"type": "string"}},
            "required": ["text"],
        }),
    },
    label: "Echo".into(), prepare: None, execution_mode: None,
    execute: Arc::new(|invocation| Box::pin(async move {
        let result = ToolResult {
            content: vec![InputContent::Text(TextContent {
                text: invocation.args["text"].as_str().unwrap().into(),
                replay_metadata: None,
            })],
            details: serde_json::json!({"call": invocation.tool_call_id}),
            terminate: Some(true),
        };
        (invocation.progress)(result.clone());
        Ok(result)
    })),
};
let agent = Agent::new(Arc::new(models), model, AgentOptions {
    tools: vec![echo], ..Default::default()
});
let input = AgentMessage::Model(Message::User(UserMessage {
    content: vec![InputContent::Text(TextContent {
        text: "Hello".into(), replay_metadata: None,
    })], timestamp: 17,
}));
let options = StreamOptions {
    auth: Some(RequestAuth::ConfiguredWithoutSecret { source: None }),
    ..Default::default()
};
let new_messages = agent.prompt(input, options)?.await?;
assert_eq!(new_messages.len(), 3);
assert!(matches!(&new_messages[2], AgentMessage::Model(Message::ToolResult(result))
    if !result.is_error && result.tool_call_id == "call"));
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
