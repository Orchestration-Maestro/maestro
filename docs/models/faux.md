### Faux provider for tests

`register_faux_provider()` registers a temporary in-memory provider for tests and demos. It is opt-in and not part of the built-in provider set.

```rust
use maestro_models::*;
use std::{future::Future, sync::{Arc, RwLock}, task::{Context as TaskContext, Poll, Wake, Waker}};

fn wait<T>(future: impl Future<Output = T>) -> T {
    struct Thread(std::thread::Thread);
    impl Wake for Thread {
        fn wake(self: Arc<Self>) { self.0.unpark(); }
    }
    let waker = Waker::from(Arc::new(Thread(std::thread::current())));
    let mut cx = TaskContext::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

let registration = register_faux_provider(RegisterFauxProviderOptions {
    tokens_per_second: Some(50.0), // optional
    ..Default::default()
});
let model = registration.get_model(None).unwrap();
let mut context = Context {
    system_prompt: None,
    messages: vec![Message::User(UserMessage {
        content: UserContent::Text("Summarize package.json and then call echo".into()),
        timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64,
    })],
    tools: None,
};
registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
    FauxAssistantContent::Blocks(vec![
        AssistantContent::Thinking(faux_thinking("Need to inspect package metadata first.".into())),
        AssistantContent::ToolCall(Arc::new(RwLock::new(faux_tool_call(
            "echo".into(),
            serde_json::json!({"text": "package.json"}).as_object().unwrap().clone(),
            FauxToolCallOptions::default(),
        )))),
    ]),
    FauxAssistantMessageOptions { stop_reason: Some(StopReason::ToolUse), ..Default::default() },
))]);
let first = wait(complete(model.clone(), context.clone(), Some(ProviderStreamOptions {
    base: StreamOptions {
        session_id: Some("session-1".into()),
        cache_retention: Some(CacheRetention::Short),
        ..Default::default()
    },
    ..Default::default()
}))).unwrap();
let first = first.read().unwrap().clone();
let tool_id = first.content.iter().find_map(|block| match block {
    AssistantContent::ToolCall(call) => Some(call.read().unwrap().id.clone()),
    _ => None,
}).unwrap();
context.messages.push(Message::Assistant(first));
context.messages.push(Message::ToolResult(ToolResultMessage {
    tool_call_id: tool_id,
    tool_name: "echo".into(),
    content: vec![InputContent::Text(faux_text("package.json contents here".into()))],
    details: None,
    is_error: false,
    timestamp: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64,
}));
registration.set_responses(vec![FauxResponseStep::Message(faux_assistant_message(
    FauxAssistantContent::Blocks(vec![
        AssistantContent::Thinking(faux_thinking("Now I can summarize the tool output.".into())),
        AssistantContent::Text(faux_text("Here is the summary.".into())),
    ]),
    FauxAssistantMessageOptions::default(),
))]);
let s = stream(model, context, None).unwrap();
let mut events = s.iter();
while let Some(event) = wait(events.next()) {
    println!("{}", serde_json::to_value(event).unwrap()["type"].as_str().unwrap());
}

// Optional: register multiple faux models for model-switching tests.
let multi_model = register_faux_provider(RegisterFauxProviderOptions {
    models: Some([("faux-fast", false), ("faux-thinker", true)].into_iter().map(|(id, reasoning)| {
        FauxModelDefinition {
            id: id.into(), name: None, reasoning: Some(reasoning), input: None,
            cost: None, context_window: None, max_tokens: None,
        }
    }).collect()),
    ..Default::default()
});
let thinker = multi_model.get_model(Some("faux-thinker"));
println!("{}", thinker.unwrap().reasoning);
println!("{}", registration.get_pending_response_count());
println!("{}", registration.state.read().unwrap().call_count);
registration.unregister();
multi_model.unregister();
```

Notes:
- Responses are consumed from a queue in request start order.
- If the queue is empty, the faux provider returns an assistant error message with `error_message: "No more faux responses queued"`.
- Use `registration.set_responses([...])` to replace the remaining queue and `registration.append_responses([...])` to add more responses.
- `registration.models` exposes all registered faux models. `registration.get_model(None)` returns the first one, and `registration.get_model(Some(id))` returns a specific one.
- Use `faux_assistant_message(...)` for scripted assistant replies. Use `faux_text(...)`, `faux_thinking(...)`, and `faux_tool_call(...)` to build content blocks without filling in low-level fields manually.
- `registration.unregister()` removes the temporary provider from the global API registry.
- Usage is estimated at roughly 1 token per 4 characters. When `session_id` is present and `cache_retention` is not `None`, prompt cache reads and writes are simulated automatically.
- Tool call arguments stream incrementally via `toolcall_delta` chunks.
- By default, each streamed chunk yields without a pacing delay. Set `tokens_per_second` to pace chunk delivery in real time.
- The intended use is one deterministic scripted flow per registration. If you need independent concurrent flows, register separate faux providers.

Defaults: provider `faux`, model `faux-1` / `Faux Model`, base URL
`http://localhost:0`, text and image input, context window 128000, maximum
output 16384, zero prices. Registration generates an isolated API identifier;
builders use API `faux`. Supplied model fields are accepted without validation.
Default chunks contain 3–5 estimated tokens, with four characters per token.
Text, single blocks, lists and empty lists are accepted. Usage and cache costs
remain zero, including for models with supplied prices. Total tokens count each
input and output token once; cache writes are already included in input.

Factories receive the context, untouched options, live shared call counter and
requested model. The response hook runs first. Exhaustion returns the error
above with estimated usage; hook and factory failures retain their supplied
error text with zero usage. Cancellation retains the streamed prefix and full
estimated usage, with `Request was aborted`. Earlier partial events are owned
snapshots; terminal events and results share one message handle. Tool arguments
remain empty until their end event.

The native host runs producers on one lazy process-wide, time-enabled Tokio
runtime, independently of the caller's executor. Browser producers use standard
local async execution; a timer callback resolves a promise, awaited through
`JsFuture`, while unpaced chunks await an already-resolved promise. Positive pacing waits for its scheduled
timer even after cancellation. If a thrown value's own string
conversion fails, the host reports the uncaught error but leaves that invocation
unsettled and keeps running. No credentials, network, bundled activation or
runtime controls are needed by callers.

Native facilities determine runtime details: generated IDs retain the
`prefix:milliseconds:base36` format but render a random integer rather than a
fraction; Unicode counts and chunks use Rust characters rather than 16-bit units,
so non-BMP text has different token counts and is never split within a character.
JSON uses plain serde serialization, keeping its number spelling, integer precision
and insertion order without integer-key sorting; floating-point min/max use Rust's NaN handling; native pacing uses Duration
and Tokio without a one-millisecond fallback, signed-32-bit ceiling or overflow
warning (unrepresentable durations panic). Browser pacing uses the browser timer
as-is. Non-error JSON failures use JSON text instead of object or array coercion,
and numeric failures use Rust number formatting. Error messages remain verbatim.
