# Agent guide

`maestro-agent` runs a conversation: it keeps the history, asks a model for each turn,
executes the tools the model requests and reports what happens as events. This guide
walks through its public interface. [Agent state and owned runs](../agent.md) and
[conversation turns](../loop.md) own the detailed contracts; the [proxy client](../proxy.md)
owns remote dispatch. Every runnable example below runs offline: model examples use a registered scripted model, and the proxy example uses a controlled fetch.

## Installation

Inside this workspace, depend on the crate by path; the model records it uses come from
`maestro-models`.

```toml
[dependencies]
maestro-agent = { path = "../maestro-agent" }
maestro-models = { path = "../maestro-models" }
```

The runnable examples also use `tokio` for the task facility the caller must supply and
`futures-util` for stream and future combinators.

## Local execution

Use `Agent::<C, Local>::new_local` inside a caller-owned local task facility when
callbacks retain single-thread objects. `AgentTool`, `AgentContext`,
`AgentLoopConfig` and `AgentLoopOptions` keep their native thread-safe struct-literal
aliases (browser targets use `Rc` callbacks and local futures); the corresponding `LocalAgent*` aliases select local callables. Generic
code can use their `*Record` owners. Agent hooks borrow mutable arguments across
awaits in either representation. Model payload and response hooks follow the same
representation: local hooks and their futures may retain `Rc` state, while the default
agent keeps thread-safe hooks on native targets (browser targets use local callbacks and
futures).

```rust
use maestro_agent::{Agent, AgentOptions, Local};
use std::{convert::Infallible, rc::Rc};

let local = Rc::new(tokio::task::LocalSet::new());
let tasks = local.clone();
let ui = Rc::new(String::from("editor"));
let retained = ui.clone();
let agent = Rc::new(Agent::<Infallible, Local>::new_local(AgentOptions {
    spawn: Some(Rc::new(move |task| { tasks.spawn_local(task); })),
    before_tool_call: Some(Rc::new(move |context, _| {
        let ui = retained.clone();
        Box::pin(async move {
            context.args["editor"] = ui.as_str().into();
            Ok(None)
        })
    })),
    ..Default::default()
}));
assert!(agent.signal().is_none());
```

The caller must drive the local task facility for admitted work to finish, just
as the native thread-safe examples below require a running task facility. See
[owned runs](../agent.md) and [raw local operations](../loop.md).

A run uses the hooks captured when it was admitted; replacing `on_payload` or
`on_response` affects later runs only. The registered provider calls `on_payload`
with its request body: the returned record's `payload` member is input to the
provider's request preparation. `on_response` observes the response status and headers
at the provider's usual point and cannot replace them. Each provider controls whether
it invokes the hooks and handles rejections under its own error and retry policy,
so a rejection does not always end the run: a failed response-session setup can be
retried successfully. When a run does end with an
error assistant message, the agent retains that message's `error_message`, not its
text content, as its error. In the default agent, hook closures need no type
annotations:

```rust
use maestro_agent::{Agent, AgentOptions};
use maestro_models::SharedHandle;

fn default_agent_hook_closures_keep_inference() -> Agent {
    Agent::new(AgentOptions {
        on_payload: Some(SharedHandle::new(|mut input, model| {
            Box::pin(async move {
                input.payload["model"] = model.id.as_str().into();
                Ok(input)
            })
        })),
        on_response: Some(SharedHandle::new(|response, _| {
            Box::pin(async move {
                assert!(response.status > 0.0);
                Ok(())
            })
        })),
        ..Default::default()
    })
}
let agent = default_agent_hook_closures_keep_inference();
assert!(agent.on_payload.read().unwrap().is_some());
assert!(agent.on_response.read().unwrap().is_some());
```

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
# use maestro_agent::{Agent, AgentInitialState, AgentMessage, AgentOptions, CustomAgentMessages, PromptInput};
# use maestro_models::{
#     BoxFuture, FauxAssistantMessageOptions, FauxResponseStep, Message, RegisterFauxProviderOptions,
#     UserBlock, UserContent, UserMessage, faux_assistant_message, register_faux_provider,
# };
# use std::sync::{Arc, Mutex, RwLock};
# fn ready<T: Send + 'static>(value: T) -> BoxFuture<T> { Box::pin(std::future::ready(value)) }
# fn plain(text: &str) -> UserMessage { UserMessage { content: UserContent::Text(text.into()), timestamp: 0.0 } }
# fn shown(message: &Message) -> String {
#     match message {
#         Message::User(UserMessage { content: UserContent::Text(text), .. }) => text.clone(),
#         Message::User(UserMessage { content: UserContent::Blocks(blocks), .. }) => blocks
#             .iter()
#             .filter_map(|block| match block { UserBlock::Text(text) => Some(text.text.as_str()), _ => None })
#             .collect(),
#         _ => "other".into(),
#     }
# }
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
struct Notification {
    text: String,
}
impl CustomAgentMessages for Notification {
    fn role(&self) -> &str {
        "notification"
    }
}
fn entry(text: &str) -> AgentMessage<Notification> {
    AgentMessage::User(Arc::new(RwLock::new(plain(text))))
}

let faux = register_faux_provider(RegisterFauxProviderOptions::default());
let requests = Arc::new(Mutex::new(Vec::new()));
let seen = Arc::clone(&requests);
struct RequestTexts(Arc<Mutex<Vec<Vec<String>>>>);
impl RequestTexts {
    fn respond<E: maestro_models::ProviderExecution>(&self, input: maestro_models::FauxResponseInput<E>)
        -> impl std::future::Future<Output = Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>> + 'static
    {
        let texts = input.context.messages.iter().map(shown).collect::<Vec<_>>();
        self.0.lock().unwrap().push(texts);
        std::future::ready(Ok(faux_assistant_message("ok", FauxAssistantMessageOptions::default())))
    }
}
impl maestro_models::Invoke<maestro_models::Local, maestro_models::FauxResponseInput<maestro_models::Local>> for RequestTexts {
    type Return = <maestro_models::Local as maestro_models::Execution>::Future<'static, Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>>;
    fn invoke(&self, input: maestro_models::FauxResponseInput<maestro_models::Local>) -> Self::Return {
        Box::pin(self.respond(input))
    }
}
impl maestro_models::Invoke<maestro_models::Threaded, maestro_models::FauxResponseInput<maestro_models::Threaded>> for RequestTexts {
    type Return = <maestro_models::Threaded as maestro_models::Execution>::Future<'static, Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>>;
    fn invoke(&self, input: maestro_models::FauxResponseInput<maestro_models::Threaded>) -> Self::Return {
        Box::pin(self.respond(input))
    }
}
faux.set_responses(vec![FauxResponseStep::Factory(Arc::new(RequestTexts(seen)))]);

let earlier_reply = AgentMessage::Assistant(Arc::new(RwLock::new(faux_assistant_message("earlier", FauxAssistantMessageOptions::default()))));
let notice = AgentMessage::Custom(Arc::new(RwLock::new(Notification { text: "Info".into() })));
let agent: Arc<Agent<Notification>> = Arc::new(Agent::new(AgentOptions {
    spawn: Some(Arc::new(|task| {
        tokio::spawn(task);
    })),
    // Standard roles pass through; the application's own entry becomes a user message.
    convert_to_llm: Some(Arc::new(|history| {
        let messages = history
            .read()
            .unwrap()
            .iter()
            .filter_map(|entry| match entry {
                AgentMessage::User(user) => Some(Message::User(user.read().unwrap().clone())),
                AgentMessage::Assistant(reply) => Some(Message::Assistant(reply.read().unwrap().clone())),
                AgentMessage::ToolResult(result) => Some(Message::ToolResult(result.read().unwrap().clone())),
                AgentMessage::Custom(notice) => {
                    Some(Message::User(plain(&format!("Notice: {}", notice.read().unwrap().text))))
                }
            })
            .collect();
        ready(Ok(messages))
    })),
    // Prune the two oldest entries before conversion.
    transform_context: Some(Arc::new(|history, _signal| {
        let recent = history.read().unwrap().iter().skip(2).cloned().collect();
        ready(Ok(Arc::new(RwLock::new(recent))))
    })),
    initial_state: AgentInitialState {
        model: faux.get_model(None).map(|model| Arc::new(RwLock::new(model.clone()))),
        messages: vec![entry("old question"), entry("old answer"), notice, entry("recent"), earlier_reply],
        ..Default::default()
    },
    ..Default::default()
}));

agent.prompt(PromptInput::Text { text: "now".into(), images: vec![] }).await.unwrap();
assert_eq!(*requests.lock().unwrap(), [["Notice: Info", "recent", "other", "now"]]);
// Pruning shaped the request only; the retained history keeps everything.
assert_eq!(agent.state().read().unwrap().messages().read().unwrap().len(), 7);
# faux.unregister();
# });
```

## Event flow

Subscribers receive an `AgentEvent`; the ten kinds are `AgentStart`, `AgentEnd`,
`TurnStart`, `TurnEnd`, `MessageStart`, `MessageUpdate`, `MessageEnd`,
`ToolExecutionStart`, `ToolExecutionUpdate` and `ToolExecutionEnd`.

With no queued steering or follow-up input, a successful prompt of one text message that calls no tools emits, in order: `AgentStart`,
`TurnStart`, the prompt's `MessageStart` and `MessageEnd`, the assistant's `MessageStart`,
a `MessageUpdate` for each model event between the start and terminal events, `MessageEnd`,
`TurnEnd` and `AgentEnd`. An empty prompt emits no prompt message pair and a batched
prompt emits one pair per message; [conversation turns](../loop.md) owns the other
branches, including failures and aborts. When the assistant calls tools, each call
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
| `initial_state.model` | a new inert descriptor named `unknown` for each agent |
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

Callback and scalar replacements apply to runs admitted afterwards. The thinking budgets
in the example below, `minimal: 128` and `high: 2048`, are illustrations, not defaults.

Values given at construction reach the next model request. The example reads them back
from the options the model receives:

```rust
# use maestro_agent::{Agent, AgentInitialState, AgentOptions, PromptInput, ThinkingLevel};
# use maestro_models::{
#     BoxFuture, FauxAssistantMessageOptions, FauxResponseOptions, FauxResponseStep,
#     RegisterFauxProviderOptions, ThinkingBudgets, Transport, faux_assistant_message,
#     register_faux_provider,
# };
# use std::sync::{Arc, Mutex, RwLock};
# fn ready<T: Send + 'static>(value: T) -> BoxFuture<T> { Box::pin(std::future::ready(value)) }
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let faux = register_faux_provider(RegisterFauxProviderOptions::default());
let received = Arc::new(std::sync::atomic::AtomicBool::new(false));
let budgets = ThinkingBudgets { minimal: Some(128.0), high: Some(2048.0), ..Default::default() };
struct CheckOptions {
    received: Arc<std::sync::atomic::AtomicBool>,
    budgets: ThinkingBudgets,
}
impl CheckOptions {
    fn respond<E: maestro_models::ProviderExecution>(&self, input: maestro_models::FauxResponseInput<E>)
        -> impl std::future::Future<Output = Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>> + 'static
    {
        let Some(FauxResponseOptions::Simple(options)) = input.options else {
            panic!("the Agent requests through the simple adapter");
        };
        assert_eq!(options.common.session_id.as_deref(), Some("session-123"));
        assert_eq!(options.common.transport, Some(Transport::Websocket));
        assert_eq!(options.common.max_retry_delay_ms, Some(1234.0));
        assert_eq!(options.common.api_key.as_deref(), Some("resolved"));
        assert_eq!(options.reasoning, Some(maestro_models::ThinkingLevel::Low));
        assert_eq!(options.thinking_budgets, Some(self.budgets.clone()));
        self.received.store(true, std::sync::atomic::Ordering::SeqCst);
        std::future::ready(Ok(faux_assistant_message("ok", FauxAssistantMessageOptions::default())))
    }
}
impl maestro_models::Invoke<maestro_models::Local, maestro_models::FauxResponseInput<maestro_models::Local>> for CheckOptions {
    type Return = <maestro_models::Local as maestro_models::Execution>::Future<'static, Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>>;
    fn invoke(&self, input: maestro_models::FauxResponseInput<maestro_models::Local>) -> Self::Return {
        Box::pin(self.respond(input))
    }
}
impl maestro_models::Invoke<maestro_models::Threaded, maestro_models::FauxResponseInput<maestro_models::Threaded>> for CheckOptions {
    type Return = <maestro_models::Threaded as maestro_models::Execution>::Future<'static, Result<maestro_models::AssistantMessage, maestro_models::DiagnosticErrorInfo>>;
    fn invoke(&self, input: maestro_models::FauxResponseInput<maestro_models::Threaded>) -> Self::Return {
        Box::pin(self.respond(input))
    }
}
faux.set_responses(vec![FauxResponseStep::Factory(Arc::new(CheckOptions {
    received: Arc::clone(&received), budgets: budgets.clone(),
}))]);

let agent: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
    spawn: Some(Arc::new(|task| {
        tokio::spawn(task);
    })),
    session_id: Some("session-123".into()),
    transport: Some(Transport::Websocket),
    max_retry_delay_ms: Some(1234.0),
    thinking_budgets: Some(Arc::new(RwLock::new(budgets.clone()))),
    get_api_key: Some(Arc::new(|_provider| ready(Ok(Some("resolved".into()))))),
    initial_state: AgentInitialState {
        model: faux.get_model(None).map(|model| Arc::new(RwLock::new(model.clone()))),
        thinking_level: Some(ThinkingLevel::Low),
        ..Default::default()
    },
    ..Default::default()
}));
agent.prompt(PromptInput::Text { text: "Hi".into(), images: vec![] }).await.unwrap();

assert!(received.load(std::sync::atomic::Ordering::SeqCst));
# faux.unregister();
# });
```

## State and control

`agent.state()` is the live `AgentState`: `system_prompt`, `model` and `thinking_level` are
plain slots; `tools()` and `messages()` are shared collections; `is_streaming()`,
`streaming_message()`, `pending_tool_calls()` and `error_message()` describe the active
run. Release state guards before calling another Agent operation.

The operations are `prompt`, `r#continue`, `abort`, `wait_for_idle` and `reset`. The
example runs a prompt, continues it from queued input, aborts a cooperating run and resets:

```rust
# use maestro_agent::{Agent, AgentInitialState, AgentMessage, AgentOptions, PromptInput};
# use maestro_models::{
#     BoxFuture, DiagnosticErrorInfo, FauxResponseStep, RegisterFauxProviderOptions, StopReason,
#     TextContent, UserBlock, UserContent, UserMessage, faux_assistant_message,
#     register_faux_provider,
# };
# use std::sync::{Arc, RwLock};
# fn text(value: &str) -> PromptInput { PromptInput::Text { text: value.into(), images: vec![] } }
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let faux = register_faux_provider(RegisterFauxProviderOptions::default());
faux.set_responses(vec![
    FauxResponseStep::Message(Box::new(faux_assistant_message("one", Default::default()))),
    FauxResponseStep::Message(Box::new(faux_assistant_message("two", Default::default()))),
]);
let spawn: maestro_agent::AgentSpawner = Arc::new(|task| {
    tokio::spawn(task);
});
let agent: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
    spawn: Some(Arc::clone(&spawn)),
    initial_state: AgentInitialState {
        model: faux.get_model(None).map(|model| Arc::new(RwLock::new(model.clone()))),
        ..Default::default()
    },
    ..Default::default()
}));

agent.prompt(text("first")).await.unwrap();
agent.wait_for_idle().await;
// An assistant tail needs queued input before it can continue.
assert!(agent.r#continue().await.is_err());
agent.follow_up(AgentMessage::User(Arc::new(RwLock::new(UserMessage {
    content: UserContent::Text("more".into()),
    timestamp: 0.0,
}))));
agent.r#continue().await.unwrap();
let length = |agent: &Agent| agent.state().read().unwrap().messages().read().unwrap().len();
assert_eq!(length(&agent), 4);

// A run waits for the stream; `abort` cancels the run's signal and the stream cooperates.
let started = Arc::new(tokio::sync::Notify::new());
let announce = Arc::clone(&started);
let stalled: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
    spawn: Some(spawn),
    stream_fn: Some(Arc::new(move |_model, _context, options, _| {
        let announce = Arc::clone(&announce);
        Box::pin(async move {
            let signal = options.common.signal.expect("every run carries a signal");
            announce.notify_one();
            signal.cancelled().await;
            Err(DiagnosticErrorInfo { name: None, message: "cancelled".into(), code: None, stack: None })
        })
    })),
    ..Default::default()
}));
let run = stalled.prompt(text("work"));
started.notified().await;
stalled.abort();
run.await.unwrap();
stalled.wait_for_idle().await;
assert!(!stalled.state().read().unwrap().is_streaming());
let Some(AgentMessage::Assistant(last)) = stalled.state().read().unwrap().messages().read().unwrap().last().cloned() else {
    panic!("the aborted run records an assistant entry");
};
assert_eq!(last.read().unwrap().stop_reason, StopReason::Aborted);

// `reset` clears history and queues.
agent.steer(AgentMessage::User(Arc::new(RwLock::new(UserMessage {
    content: UserContent::Text("queued".into()),
    timestamp: 0.0,
}))));
agent.reset();
assert_eq!(length(&agent), 0);
assert!(!agent.has_queued_messages());
# faux.unregister();
# });
```

`continue` refuses a busy Agent, an empty history, and an assistant tail unless input is
queued (steering takes precedence). Dropping the observation returned by `prompt` does
not cancel the run.

## Subscriptions

`subscribe` registers a listener `(event, signal) → future`; listeners run in
registration order and each is awaited before the next. The returned `Subscription`
removes the listener only through an explicit `unsubscribe`. The signal is the run's
cancellation signal.

A listener that is still working on `AgentEnd` keeps the run unsettled. The example holds
one open, watches the Agent stay busy, then unsubscribes a counting listener and shows it
is no longer called:

```rust
# use futures_util::FutureExt;
# use maestro_agent::{Agent, AgentEvent, AgentInitialState, AgentOptions, PromptInput};
# use maestro_models::{FauxResponseStep, RegisterFauxProviderOptions, faux_assistant_message, register_faux_provider};
# use std::sync::{Arc, Mutex, RwLock};
# fn text(value: &str) -> PromptInput { PromptInput::Text { text: value.into(), images: vec![] } }
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let faux = register_faux_provider(RegisterFauxProviderOptions::default());
let answer = |text: &str| FauxResponseStep::Message(Box::new(faux_assistant_message(text, Default::default())));
faux.set_responses(vec![answer("one"), answer("two")]);
let agent: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
    spawn: Some(Arc::new(|task| {
        tokio::spawn(task);
    })),
    initial_state: AgentInitialState {
        model: faux.get_model(None).map(|model| Arc::new(RwLock::new(model.clone()))),
        ..Default::default()
    },
    ..Default::default()
}));

let entered = Arc::new(tokio::sync::Notify::new());
let release = Arc::new(tokio::sync::Notify::new());
let (hold_entered, hold_release) = (Arc::clone(&entered), Arc::clone(&release));
agent.subscribe(Arc::new(move |event, _signal| {
    let ending = matches!(&*event.read().unwrap(), AgentEvent::AgentEnd { .. });
    let (entered, release) = (Arc::clone(&hold_entered), Arc::clone(&hold_release));
    Box::pin(async move {
        if ending {
            entered.notify_one();
            release.notified().await;
        }
        Ok(())
    })
}));
let calls = Arc::new(Mutex::new(0));
let tally = Arc::clone(&calls);
let counter = agent.subscribe(Arc::new(move |_event, _signal| {
    *tally.lock().unwrap() += 1;
    Box::pin(std::future::ready(Ok(())))
}));

let run = agent.prompt(text("first"));
entered.notified().await;
// `AgentEnd` was delivered, but its slow listener keeps the Agent unsettled.
assert!(agent.wait_for_idle().now_or_never().is_none());
release.notify_one();
run.await.unwrap();
assert!(agent.wait_for_idle().now_or_never().is_some());
let delivered = *calls.lock().unwrap();
assert!(delivered > 0);

counter.unsubscribe();
let run = agent.prompt(text("second"));
entered.notified().await;
release.notify_one();
run.await.unwrap();
assert_eq!(*calls.lock().unwrap(), delivered);
# faux.unregister();
# });
```

## Steering and follow-up

`steer` stores input for the next turn boundary; `follow_up` stores input for when the
model would otherwise stop. Each queue drains by its own mode: `All` moves everything,
`OneAtATime` the front entry. Follow-ups are polled only when the batch needs no
continuation and no steering is queued. `clear_steering_queue`, `clear_follow_up_queue`
and `clear_all_queues` drop queued input.

The example runs the same queued input under unequal modes in both directions, then
clears one queue and keeps the other:

```rust
# use maestro_agent::{Agent, AgentInitialState, AgentMessage, AgentOptions, PromptInput, QueueMode};
# use maestro_models::{FauxResponseStep, RegisterFauxProviderOptions, UserBlock, UserContent, UserMessage, faux_assistant_message, register_faux_provider};
# use std::sync::{Arc, RwLock};
# fn text(value: &str) -> PromptInput { PromptInput::Text { text: value.into(), images: vec![] } }
# fn queued(value: &str) -> AgentMessage {
#     AgentMessage::User(Arc::new(RwLock::new(UserMessage { content: UserContent::Text(value.into()), timestamp: 0.0 })))
# }
# fn shown(agent: &Agent) -> Vec<String> {
#     let state = agent.state().read().unwrap();
#     let messages = state.messages().read().unwrap();
#     messages.iter().map(|entry| match entry {
#         AgentMessage::User(user) => match &user.read().unwrap().content {
#             UserContent::Text(text) => text.clone(),
#             UserContent::Blocks(blocks) => blocks.iter().filter_map(|block| match block { UserBlock::Text(t) => Some(t.text.clone()), _ => None }).collect(),
#         },
#         AgentMessage::Assistant(reply) => match &reply.read().unwrap().content[0] {
#             maestro_models::AssistantContent::Text(text) => text.text.clone(),
#             _ => "other".into(),
#         },
#         _ => "other".into(),
#     }).collect()
# }
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let faux = register_faux_provider(RegisterFauxProviderOptions::default());
let model = faux.get_model(None).map(|model| Arc::new(RwLock::new(model.clone())));
let run = |steering: QueueMode, follow_up: QueueMode| {
    faux.set_responses(["r1", "r2", "r3"].map(|text| {
        FauxResponseStep::Message(Box::new(faux_assistant_message(text, Default::default())))
    }).to_vec());
    let agent: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
        spawn: Some(Arc::new(|task| {
            tokio::spawn(task);
        })),
        steering_mode: Some(steering),
        follow_up_mode: Some(follow_up),
        initial_state: AgentInitialState { model: model.clone(), ..Default::default() },
        ..Default::default()
    }));
    agent.steer(queued("s1"));
    agent.steer(queued("s2"));
    agent.follow_up(queued("f1"));
    agent.follow_up(queued("f2"));
    agent
};

let agent = run(QueueMode::All, QueueMode::OneAtATime);
agent.prompt(text("P")).await.unwrap();
assert_eq!(shown(&agent), ["P", "s1", "s2", "r1", "f1", "r2", "f2", "r3"]);

let agent = run(QueueMode::OneAtATime, QueueMode::All);
agent.prompt(text("P")).await.unwrap();
assert_eq!(shown(&agent), ["P", "s1", "r1", "s2", "r2", "f1", "f2", "r3"]);

let agent = run(QueueMode::All, QueueMode::All);
agent.clear_steering_queue();
assert!(agent.has_queued_messages(), "the follow-up queue is untouched");
agent.prompt(text("P")).await.unwrap();
assert_eq!(shown(&agent), ["P", "r1", "f1", "f2", "r2"]);
# faux.unregister();
# });
```

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
        })
        .into(),
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
[`ProxyStreamOptions`](../proxy.md) from the options the Agent passes in.

The example answers from a controlled `Fetch` placed in the stream options, so no
backend is contacted, and checks both the requested address and the reply:

```rust
# use maestro_agent::{Agent, AgentMessage, AgentOptions, PromptInput, ProxyStreamOptions, stream_proxy};
# use maestro_models::{AssistantContent, HttpResponse};
# use std::sync::{Arc, Mutex};
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let spawn: maestro_agent::AgentSpawner = Arc::new(|task| {
    tokio::spawn(task);
});

// The backend's reply: one text block, then a terminal event.
let reply: Vec<Vec<u8>> = [
    r#"{"type":"start"}"#,
    r#"{"type":"text_start","contentIndex":0}"#,
    r#"{"type":"text_delta","contentIndex":0,"delta":"Hello from the proxy"}"#,
    r#"{"type":"text_end","contentIndex":0}"#,
    r#"{"type":"done","reason":"stop","usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"totalTokens":2,"cost":{"input":0,"output":0,"cacheRead":0,"cacheWrite":0,"total":0}}}"#,
]
.iter()
.map(|payload| format!("data: {payload}\n").into_bytes())
.collect();
let urls = Arc::new(Mutex::new(Vec::new()));
let seen = Arc::clone(&urls);
let fetch: maestro_models::Fetch = Arc::new(move |request| {
    seen.lock().unwrap().push(request.url);
    let body = futures_util::stream::iter(reply.clone().into_iter().map(Ok));
    Box::pin(async move {
        Ok(HttpResponse {
            status: 200,
            status_text: "OK".into(),
            headers: Default::default(),
            body: Box::pin(body),
        })
    })
});

let proxy_spawn = Arc::clone(&spawn);
let agent: Arc<Agent> = Arc::new(Agent::new(AgentOptions {
    spawn: Some(spawn),
    stream_fn: Some(Arc::new(move |model, context, mut options, _| {
        options.common.fetch = Some(Arc::clone(&fetch));
        let proxy = ProxyStreamOptions {
            options,
            auth_token: "local-credential".into(),
            proxy_url: "https://proxy.example".into(),
        };
        let stream = stream_proxy(model, context, proxy, Arc::clone(&proxy_spawn));
        Box::pin(std::future::ready(Ok(stream)))
    })),
    ..Default::default()
}));
agent.prompt(PromptInput::Text { text: "Hi".into(), images: vec![] }).await.unwrap();

assert_eq!(*urls.lock().unwrap(), ["https://proxy.example/api/stream"]);
let state = agent.state().read().unwrap();
let messages = state.messages().read().unwrap();
let Some(AgentMessage::Assistant(reply)) = messages.last() else { panic!("assistant reply") };
let AssistantContent::Text(text) = &reply.read().unwrap().content[0] else { panic!("text reply") };
assert_eq!(text.text, "Hello from the proxy");
# });
```

## Raw loops

Without the `Agent`, four entry points take an `AgentContext` and an `AgentLoopConfig`:
`run_agent_loop` and `run_agent_loop_continue` are awaited and report each event through a
sink you await; `agent_loop` and `agent_loop_continue` submit the work through a spawner
and return an observable `EventStream`. The streams are observational: they keep event
order but do not wait for your handling before the producer continues. Use `Agent` when
message processing must finish before tool preflight.

The example runs all four entry points against a scripted model. The awaited loops hand
each event to a sink and return the entries they generated; the observable loops return
at once, and their result settles without any reader consuming the events:

```rust
# use maestro_agent::{
#     AgentContext, AgentEvent, AgentLoopConfig, AgentLoopOptions, AgentMessage, agent_loop,
#     agent_loop_continue, run_agent_loop, run_agent_loop_continue,
# };
# use maestro_models::{
#     BoxFuture, FauxResponseStep, JsonObject, Message, ProviderObjects, RegisterFauxProviderOptions,
#     SimpleStreamOptions, UserContent, UserMessage, faux_assistant_message, register_faux_provider,
# };
# use std::sync::{Arc, Mutex, RwLock};
# fn ready<T: Send + 'static>(value: T) -> BoxFuture<T> { Box::pin(std::future::ready(value)) }
# fn entry(text: &str) -> AgentMessage {
#     AgentMessage::User(Arc::new(RwLock::new(UserMessage { content: UserContent::Text(text.into()), timestamp: 0.0 })))
# }
# fn said(messages: &[AgentMessage]) -> Vec<String> {
#     messages.iter().map(|entry| match entry {
#         AgentMessage::User(user) => match &user.read().unwrap().content {
#             UserContent::Text(text) => format!("user:{text}"),
#             _ => "user:other".into(),
#         },
#         AgentMessage::Assistant(reply) => match &reply.read().unwrap().content[0] {
#             maestro_models::AssistantContent::Text(text) => format!("assistant:{}", text.text),
#             _ => "assistant:other".into(),
#         },
#         _ => "other".into(),
#     }).collect()
# }
# fn label(event: &AgentEvent) -> &'static str {
#     match event {
#         AgentEvent::AgentStart => "agent_start",
#         AgentEvent::AgentEnd { .. } => "agent_end",
#         AgentEvent::TurnStart => "turn_start",
#         AgentEvent::TurnEnd { .. } => "turn_end",
#         AgentEvent::MessageStart { .. } => "message_start",
#         AgentEvent::MessageUpdate { .. } => "message_update",
#         AgentEvent::MessageEnd { .. } => "message_end",
#         _ => "tool",
#     }
# }
# fn config(model: &maestro_models::Model) -> AgentLoopConfig {
#     AgentLoopConfig {
#         before_tool_call: None, after_tool_call: None, should_stop_after_turn: None, prepare_next_turn: None,
#         get_steering_messages: None, get_follow_up_messages: None, tool_execution: None,
#         model: Arc::new(RwLock::new(model.clone())), thinking_budgets: None,
#         options: SimpleStreamOptions::default(), extra: JsonObject::new(), objects: ProviderObjects::default(),
#         convert_to_llm: Arc::new(|history| {
#             let messages = history.read().unwrap().iter().filter_map(|entry| match entry {
#                 AgentMessage::User(user) => Some(Message::User(user.read().unwrap().clone())),
#                 AgentMessage::Assistant(reply) => Some(Message::Assistant(reply.read().unwrap().clone())),
#                 AgentMessage::ToolResult(result) => Some(Message::ToolResult(result.read().unwrap().clone())),
#                 AgentMessage::Custom(never) => match *never.read().unwrap() {},
#             }).collect();
#             ready(Ok(messages))
#         }),
#         transform_context: None, get_api_key: None,
#     }
# }
# fn context(history: Vec<AgentMessage>) -> AgentContext {
#     AgentContext { system_prompt: String::new(), messages: Arc::new(RwLock::new(history)), tools: None }
# }
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let faux = register_faux_provider(RegisterFauxProviderOptions::default());
let answer = |text: &str| FauxResponseStep::Message(Box::new(faux_assistant_message(text, Default::default())));
faux.set_responses(vec![answer("a1"), answer("a2"), answer("a3"), answer("a4")]);
let model = faux.get_model(None).unwrap();
let config = config(model);
let spawn: maestro_agent::AgentSpawner = Arc::new(|task| {
    tokio::spawn(task);
});
let prompt_run = [
    "agent_start", "turn_start", "message_start", "message_end", "message_start",
    "message_end", "turn_end", "agent_end",
];
let continue_run = [
    "agent_start", "turn_start", "message_start", "message_end", "turn_end", "agent_end",
];
// The number of `MessageUpdate` events follows the model's chunking, so they are left out.
let interesting = |event: &AgentEvent| !matches!(event, AgentEvent::MessageUpdate { .. });

// Awaited: the sink finishes each event before the loop moves on.
let labels = Arc::new(Mutex::new(Vec::new()));
let sink = Arc::clone(&labels);
let emit: maestro_agent::AgentEventSink = Arc::new(move |event| {
    if interesting(&event) {
        sink.lock().unwrap().push(label(&event));
    }
    ready(Ok(()))
});
let generated = run_agent_loop(vec![entry("p")], context(vec![]), &config, emit, AgentLoopOptions::default())
    .await
    .unwrap();
assert_eq!(said(&generated), ["user:p", "assistant:a1"]);
assert_eq!(*labels.lock().unwrap(), prompt_run);

let labels = Arc::new(Mutex::new(Vec::new()));
let sink = Arc::clone(&labels);
let emit: maestro_agent::AgentEventSink = Arc::new(move |event| {
    if interesting(&event) {
        sink.lock().unwrap().push(label(&event));
    }
    ready(Ok(()))
});
let generated = run_agent_loop_continue(context(vec![entry("question")]), &config, emit, AgentLoopOptions::default())
    .await
    .unwrap();
assert_eq!(said(&generated), ["assistant:a2"]);
assert_eq!(*labels.lock().unwrap(), continue_run);

// Observable: the result settles with nobody reading the events, which stay buffered.
let config = Arc::new(config);
let stream = agent_loop(vec![entry("p")], context(vec![]), Arc::clone(&config), AgentLoopOptions::default(), Arc::clone(&spawn));
assert_eq!(said(&stream.result().await), ["user:p", "assistant:a3"]);
let mut observed = Vec::new();
while let Some(event) = stream.next().await {
    if interesting(&event) {
        observed.push(label(&event));
    }
}
assert_eq!(observed, prompt_run);

let refused = agent_loop_continue(context(vec![]), Arc::clone(&config), AgentLoopOptions::default(), Arc::clone(&spawn));
assert_eq!(refused.err().unwrap().message, "Cannot continue: no messages in context");
let stream = agent_loop_continue(context(vec![entry("question")]), config, AgentLoopOptions::default(), spawn).unwrap();
assert_eq!(said(&stream.result().await), ["assistant:a4"]);
let mut observed = Vec::new();
while let Some(event) = stream.next().await {
    if interesting(&event) {
        observed.push(label(&event));
    }
}
assert_eq!(observed, continue_run);
# faux.unregister();
# });
```

## Preparing the next turn

`AgentLoopConfig::prepare_next_turn` runs after each normally completed turn, before the stop
decision. It receives the same turn record as the stop callback and the run's optional
cancellation signal as a second argument. The signal reports an abort during the turn.
The hook may return an `AgentLoopTurnUpdate` holding a replacement context
(instructions, history handle and tools), a model and a thinking level. The next request
uses them; the request already sent keeps its payload, and an absent part keeps the
current one. `Agent::prepare_next_turn` holds the callback
for runs admitted later. The application session calls an already installed hook first,
with the same turn and signal, and propagates its error. It keeps the returned context
(or the turn's context), including its messages, then refreshes the active tools,
the run's system prompt (an empty one included), the selected model and thinking level.

## Stopping after a turn

`AgentLoopConfig::should_stop_after_turn` runs after each normally completed turn, once its
`TurnEnd` has been delivered; a turn that ends in an error or abort skips it. Returning
`true` ends the run before any follow-up poll or next request, and the assistant's stop reason is unchanged. The steering poll at entry
still happens. The example records the order:

```rust
# use maestro_agent::{
#     AgentContext, AgentEvent, AgentLoopConfig, AgentLoopOptions, AgentMessage, run_agent_loop,
# };
# use maestro_models::{
#     BoxFuture, FauxResponseStep, JsonObject, Message, ProviderObjects, RegisterFauxProviderOptions,
#     SimpleStreamOptions, StopReason, UserContent, UserMessage, faux_assistant_message, register_faux_provider,
# };
# use std::sync::{Arc, Mutex, RwLock};
# fn ready<T: Send + 'static>(value: T) -> BoxFuture<T> { Box::pin(std::future::ready(value)) }
# fn entry(text: &str) -> AgentMessage {
#     AgentMessage::User(Arc::new(RwLock::new(UserMessage { content: UserContent::Text(text.into()), timestamp: 0.0 })))
# }
# fn config(model: &maestro_models::Model) -> AgentLoopConfig {
#     AgentLoopConfig {
#         before_tool_call: None, after_tool_call: None, should_stop_after_turn: None, prepare_next_turn: None,
#         get_steering_messages: None, get_follow_up_messages: None, tool_execution: None,
#         model: Arc::new(RwLock::new(model.clone())), thinking_budgets: None,
#         options: SimpleStreamOptions::default(), extra: JsonObject::new(), objects: ProviderObjects::default(),
#         convert_to_llm: Arc::new(|history| {
#             let messages = history.read().unwrap().iter().filter_map(|entry| match entry {
#                 AgentMessage::User(user) => Some(Message::User(user.read().unwrap().clone())),
#                 AgentMessage::Assistant(reply) => Some(Message::Assistant(reply.read().unwrap().clone())),
#                 AgentMessage::ToolResult(result) => Some(Message::ToolResult(result.read().unwrap().clone())),
#                 AgentMessage::Custom(never) => match *never.read().unwrap() {},
#             }).collect();
#             ready(Ok(messages))
#         }),
#         transform_context: None, get_api_key: None,
#     }
# }
# tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
let faux = register_faux_provider(RegisterFauxProviderOptions::default());
faux.set_responses(vec![FauxResponseStep::Message(Box::new(faux_assistant_message("done", Default::default())))]);
let trace = Arc::new(Mutex::new(Vec::new()));
let mut config = config(faux.get_model(None).unwrap());

let log = Arc::clone(&trace);
config.get_steering_messages = Some(Arc::new(move || {
    log.lock().unwrap().push("steering");
    ready(Ok(vec![]))
}));
let log = Arc::clone(&trace);
config.should_stop_after_turn = Some(Arc::new(move |turn| {
    log.lock().unwrap().push("stop");
    let stop_reason = turn.message.read().unwrap().stop_reason.clone();
    assert_eq!(stop_reason, StopReason::Stop);
    Box::pin(async { Ok(true) })
}));
config.get_follow_up_messages = Some(Arc::new(|| panic!("the run stopped before any follow-up poll")));

let log = Arc::clone(&trace);
let emit: maestro_agent::AgentEventSink = Arc::new(move |event| {
    if matches!(event, AgentEvent::TurnEnd { .. }) {
        log.lock().unwrap().push("turn_end");
    }
    ready(Ok(()))
});
let context = AgentContext { system_prompt: String::new(), messages: Arc::new(RwLock::new(vec![])), tools: None };
let generated = run_agent_loop(vec![entry("go")], context, &config, emit, AgentLoopOptions::default()).await.unwrap();
assert_eq!(generated.len(), 2);
assert_eq!(*trace.lock().unwrap(), ["steering", "turn_end", "stop"]);
# faux.unregister();
# });
```

## Message projection

`AgentMessage::to_message` returns an owned user, assistant or tool-result snapshot,
or `None` for custom entries. The default agent converter uses this operation.
The shared handle remains available for later edits; the returned model record is
an independent snapshot. Poisoned read locks recover their retained values.

`CustomAgentMessages::role` is a read-only accessor called under the message's
read lock. It must not access that same lock or call back into the agent.
Browser attachment records and conversion live in the
[application](../application/browser-attachments.md).
