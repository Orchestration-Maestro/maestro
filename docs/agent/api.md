# Agent guide

`maestro-agent` runs a conversation: it keeps the history, asks a model for each turn,
executes the tools the model requests and reports what happens as events. This guide
walks through its public interface. [Agent state and owned runs](../agent.md) and
[conversation turns](../loop.md) own the detailed contracts; the [proxy client](../proxy.md)
owns remote dispatch. Every example runs offline: model examples use a registered scripted model, and the proxy example uses a controlled fetch.

## Installation

Inside this workspace, depend on the crate by path; the model records it uses come from
`maestro-models`.

```toml
[dependencies]
maestro-agent = { path = "../maestro-agent" }
maestro-models = { path = "../maestro-models" }
```

The runnable examples also use `tokio` for the task facility the caller must supply.

## Quick start

An `Agent` never starts tasks by itself: `AgentOptions::spawn` receives each run's
future, and the caller drives it. The examples hand it to Tokio.

```rust
# use maestro_agent::{Agent, AgentEvent, AgentInitialState, AgentOptions, PromptInput};
# use maestro_models::{
#     AssistantMessageEvent, FauxAssistantMessageOptions, FauxResponseStep,
#     RegisterFauxProviderOptions, faux_assistant_message, register_faux_provider,
# };
# use std::sync::{Arc, Mutex, RwLock};
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let faux = register_faux_provider(RegisterFauxProviderOptions::default());
let answer = faux_assistant_message("Hello!", FauxAssistantMessageOptions::default());
faux.set_responses(vec![FauxResponseStep::Message(Box::new(answer))]);
let model = faux.get_model(None).map(|model| Arc::new(RwLock::new(model.clone())));

let agent: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
    spawn: Some(Arc::new(|task| {
        tokio::spawn(task);
    })),
    initial_state: AgentInitialState {
        system_prompt: Some("You are a helpful assistant.".into()),
        model,
        ..Default::default()
    },
    ..Default::default()
}));

let streamed = Arc::new(Mutex::new(String::new()));
let sink = Arc::clone(&streamed);
agent.subscribe(Arc::new(move |event, _signal| {
    if let AgentEvent::MessageUpdate {
        assistant_message_event: AssistantMessageEvent::TextDelta { delta, .. },
        ..
    } = &*event.read().unwrap()
    {
        sink.lock().unwrap().push_str(delta);
    }
    Box::pin(std::future::ready(Ok(())))
}));

agent
    .prompt(PromptInput::Text { text: "Hi".into(), images: vec![] })
    .await
    .unwrap();
assert_eq!(*streamed.lock().unwrap(), "Hello!");
# faux.unregister();
# });
```

## Messages and conversion

History entries are `AgentMessage` values: `User`, `Assistant`, `ToolResult`, or a
`Custom` entry of the type parameter `C: CustomAgentMessages`. A model only understands
the first three, so each request passes the history through two steps:

```text
history → transform_context (optional) → convert_to_llm → Vec<Message> → model
```

`transform_context` may prune or extend the entries (it returns the history to
convert); `convert_to_llm` turns entries into model messages. The default conversion keeps
the three standard roles and drops `Custom` entries. Both run for every request, in that
order, so the model sees only what the converter returns.

```rust
use maestro_agent::{Agent, AgentMessage, AgentOptions, CustomAgentMessages};
use std::sync::{Arc, RwLock};

struct Notification {
    text: String,
}
impl CustomAgentMessages for Notification {
    fn role(&self) -> &str {
        "notification"
    }
}

let agent: Agent<Notification> = Agent::new(AgentOptions::default());
agent.steer(AgentMessage::Custom(Arc::new(RwLock::new(Notification {
    text: "Info".into(),
}))));
assert!(agent.has_queued_messages());
```

## Event flow

Subscribers receive an `AgentEvent`; the ten kinds are `AgentStart`, `AgentEnd`,
`TurnStart`, `TurnEnd`, `MessageStart`, `MessageUpdate`, `MessageEnd`,
`ToolExecutionStart`, `ToolExecutionUpdate` and `ToolExecutionEnd`.

`prompt` emits, in order: `AgentStart`, `TurnStart`, the prompt's `MessageStart` and
`MessageEnd`, the assistant's `MessageStart`, a `MessageUpdate` for each model event between the start and terminal events
(see [conversation turns](../loop.md)), `MessageEnd`, `TurnEnd` and `AgentEnd`. When the assistant calls tools, each call
produces `ToolExecutionStart`, optional `ToolExecutionUpdate`s and `ToolExecutionEnd`, and
the tool-result message events and a `TurnEnd` that carries the results follow (sequential
tools interleave with their calls); whether the model is then asked again in a new turn is
decided by [conversation turns](../loop.md). `continue` resumes from the existing history. A user or tool-result tail adds no message
events for existing entries; an assistant tail restarts from the queued input, which is
inserted before the reply.

`MessageUpdate` carries the original model event with its shared message handle. `AgentEnd`
means no more loop events follow, but `prompt` and `wait_for_idle` settle only after
every listener finished its work for it.

## Options and defaults

| Option | Default |
| --- | --- |
| `initial_state.system_prompt` | empty text |
| `initial_state.model` | an inert descriptor named `unknown` |
| `initial_state.thinking_level` | `Off` |
| `initial_state.tools`, `initial_state.messages` | empty |
| `convert_to_llm` | standard roles kept, `Custom` dropped |
| `transform_context`, `get_api_key`, hooks, `on_payload`, `on_response` | none |
| `steering_mode`, `follow_up_mode` | `QueueMode::OneAtATime` |
| `tool_execution` | `ToolExecutionMode::Parallel` |
| `transport` | `Transport::Auto` |
| `session_id`, `max_retry_delay_ms`, `thinking_budgets` | none, forwarded unchanged when set |
| `stream_fn` | none: the registered simple adapter of the model's API |
| `spawn` | none: prompting is refused until one is supplied |

Callback and scalar replacements apply to runs admitted afterwards. Thinking budgets in
examples such as `minimal: 128, high: 2048` are illustrations, not defaults.

## State and control

`agent.state()` is the live `AgentState`: `system_prompt`, `model` and `thinking_level` are
plain slots; `tools()` and `messages()` are shared collections; `is_streaming()`,
`streaming_message()`, `pending_tool_calls()` and `error_message()` describe the active
run. Release state guards before calling another Agent operation.

```text
agent.prompt(input).await      run a prompt (text, text with images, or messages)
agent.r#continue().await       resume from the current history
agent.abort()                  cancel cooperatively
agent.wait_for_idle().await    observe completion of the active run
agent.reset()                  clear history, queues and run observations
```

`continue` refuses a busy Agent, an empty history, and an assistant tail unless input is
queued (steering takes precedence). Dropping the observation returned by `prompt` does
not cancel the run.

## Subscriptions

`subscribe` registers a listener `(event, signal) → future`; listeners run in
registration order and each is awaited before the next. The returned `Subscription`
removes the listener only through an explicit `unsubscribe`. The signal is the run's
cancellation signal.

## Steering and follow-up

`steer` stores input for the next turn boundary; `follow_up` stores input for when the
model would otherwise stop. Each queue drains by its own mode: `All` moves everything,
`OneAtATime` the front entry. Follow-ups are polled only when the batch needs no
continuation and no steering is queued. `clear_steering_queue`, `clear_follow_up_queue`
and `clear_all_queues` drop queued input.

## Tools, hooks and errors

`AgentTool::typed` wraps a typed callback: arguments are decoded from the call's JSON,
and a decoding failure never runs the callback. A tool reports failure by returning
`Err`; the Agent turns it into a tool result with `is_error` set. It reports progress
through the update callback and may set `terminate: Some(true)` on its result.

```rust
use maestro_agent::{AgentTool, AgentToolResult};
use maestro_models::{BoxFuture, TextContent, Tool, UserBlock};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
struct Echo {
    text: String,
}

let tool = AgentTool::typed(
    Tool {
        name: "echo".into(),
        description: "Repeat the text".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": {"text": {"type": "string"}},
            "required": ["text"]
        }),
    },
    "Echo".into(),
    Arc::new(|_id, params: Echo, _signal, _progress| -> BoxFuture<_> {
        Box::pin(std::future::ready(Ok(AgentToolResult {
            content: vec![UserBlock::Text(TextContent { text: params.text, text_signature: None })],
            details: Some(()),
            terminate: Some(true),
        })))
    }),
);
assert_eq!(tool.label, "Echo");
```

Calls of one assistant message run in parallel by default; a tool whose `execution_mode`
is `Sequential` makes the whole batch sequential. `before_tool_call` sees validated
arguments, may edit them and may block with a reason (`Tool execution was blocked` when
none is given). `after_tool_call` may replace content, details, the error flag and the
termination flag. A batch whose results all request termination does not ask the model again by itself. Details are in [conversation turns](../loop.md).

## Proxy use

`stream_proxy` is a ready-made `stream_fn` body for applications that reach models
through their own backend. Wrap it in a closure that builds the
[`ProxyStreamOptions`](../proxy.md) from the options the Agent passes in:

```text
stream_fn = |model, context, options, _| {
    stream_proxy(model, context, ProxyStreamOptions { options, auth_token, proxy_url }, spawn)
}
```

## Raw loops

Without the `Agent`, four entry points take an `AgentContext` and an `AgentLoopConfig`:
`run_agent_loop` and `run_agent_loop_continue` are awaited and report each event through a
sink you await; `agent_loop` and `agent_loop_continue` submit the work through a spawner
and return an observable `EventStream`. The streams are observational: they keep event
order but do not wait for your handling before the producer continues. Use `Agent` when
message processing must finish before tool preflight.
