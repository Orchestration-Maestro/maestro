# Scripted faux models

`register_faux_provider` adds a temporary in-memory provider for tests and demos.
You opt in to it; it is not one of the built-in providers. Callers reach it
through the ordinary `complete`, `stream` and simple invocation interfaces.

## Defaults

Registration uses the provider `faux` and a separately generated `faux` API
identifier. When the model list is omitted or empty, registration creates
`faux-1`, named `Faux Model`, with text and image inputs, reasoning disabled and
the URL `http://localhost:0`. Every price (input, output, cache-read and
cache-write) is zero. The context window is 128,000 tokens and the maximum output
is 16,384 tokens. A custom model's name defaults to its model ID, and any other
omitted field takes the defaults above.

`faux_assistant_message` defaults to the API and provider `faux`, the model
`faux-1`, the stop reason `Stop` and the current epoch timestamp in integer
milliseconds. Response IDs and error messages stay absent unless you supply them;
usage and costs start at zero. `faux_tool_call` generates a `tool` identifier
unless you supply one, and it keeps the argument value as given, whatever its root
kind. Text, thinking and tool content carry no signatures, and thinking carries no
redacted flag.

## Two-turn tool flow

```rust
use maestro_models::*;

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let registration = register_faux_provider(RegisterFauxProviderOptions {
    tokens_per_second: Some(50.0),
    ..RegisterFauxProviderOptions::default()
});
let model = registration.get_model(None).ok_or("missing model")?.clone();
let mut context = Context {
    system_prompt: None,
    messages: vec![Message::User(UserMessage {
        content: UserContent::Text("Summarize package.json and then call echo".into()),
        timestamp: 0.0,
    })],
    tools: None,
};
let arguments = serde_json::json!({"text": "package.json"});
registration.set_responses(vec![FauxResponseStep::Message(Box::new(
    faux_assistant_message(vec![
        AssistantContent::Thinking(faux_thinking("Need to inspect package metadata first.")),
        AssistantContent::ToolCall(faux_tool_call("echo", arguments, FauxToolCallOptions::default())),
    ], FauxAssistantMessageOptions {
        stop_reason: Some(StopReason::ToolUse),
        ..FauxAssistantMessageOptions::default()
    }),
))]);
let first = complete(model.clone(), context.clone(), Some(ProviderStreamOptions {
    common: StreamOptions {
        session_id: Some("session-1".into()),
        cache_retention: Some(CacheRetention::Short),
        ..StreamOptions::default()
    },
    ..ProviderStreamOptions::default()
})).await?;
let first = first.read().map_err(|_| "message lock poisoned")?.clone();
let tool_id = first.content.iter().find_map(|block| match block {
    AssistantContent::ToolCall(tool) => Some(tool.id.clone()),
    _ => None,
}).ok_or("missing tool")?;
context.messages.push(Message::Assistant(first));
context.messages.push(Message::ToolResult(ToolResultMessage {
    tool_call_id: tool_id,
    tool_name: "echo".into(),
    content: vec![UserBlock::Text(faux_text("package.json contents here"))],
    details: None,
    is_error: false,
    timestamp: 0.0,
}));
registration.set_responses(vec![FauxResponseStep::Message(Box::new(
    faux_assistant_message(vec![
        AssistantContent::Thinking(faux_thinking("Now I can summarize the tool output.")),
        AssistantContent::Text(faux_text("Here is the summary.")),
    ], FauxAssistantMessageOptions::default()),
))]);
let events = stream(model, context, None)?;
while let Some(event) = events.next().await {
    println!("{}", serde_json::to_value(event)?["type"].as_str().ok_or("missing event type")?);
}
let multi_model = register_faux_provider(RegisterFauxProviderOptions {
    models: Some(vec![
        FauxModelDefinition { id: "faux-fast".into(), reasoning: Some(false), ..FauxModelDefinition::default() },
        FauxModelDefinition { id: "faux-thinker".into(), reasoning: Some(true), ..FauxModelDefinition::default() },
    ]),
    ..RegisterFauxProviderOptions::default()
});
println!("{}", multi_model.get_model(Some("faux-thinker")).is_some_and(|model| model.reasoning));
println!("{}", registration.get_pending_response_count());
println!("{}", registration.state.call_count());
registration.unregister();
multi_model.unregister();
# Ok(())
# }
```

The second turn prints the event names in order: start, thinking, text and done.
The last three lines are `true`, `0`, `2`. Run the async example on your own
executor. Natively, the default producers use their own runtime with timers
enabled; in a browser they use local futures. Mode-aware registered calls use the
admission facility you supply instead. The example needs no network credentials
and no server.

## Behavior

- Responses are consumed in the order requests start (FIFO), already-aborted
  requests included. When the queue is empty, the call returns
  `No more faux responses queued` with estimated prompt usage. A factory or
  response-hook failure carries the supplied error text and zero usage instead.
  The response hook receives status 200 and empty headers before the factory
  resolves, even on exhaustion. The simulator never calls payload hooks; a
  factory may call the original hooks it receives.
- `set_responses` replaces the remaining queue and `append_responses` adds to it.
  Neither resets the call count or the cache. `state.call_count()` stays live
  across async factories. A factory receives the owned context, the raw or simple
  options and the selected model.
- `models` exposes every descriptor. `get_model(None)` and `Some("")` return the
  first, and an unknown nonempty identifier returns `None`. Optional overrides
  keep explicit empty, false and zero values. An empty model list uses the default
  model.
- `faux_assistant_message` accepts text, one block or ordered blocks. The text,
  thinking and tool builders produce unsigned content, and every message owns
  fresh zero usage. Metadata stays as supplied, while invocation rewrites the
  API, provider and model.
- `unregister` removes only the source of this registration. A registration that
  replaced it on the same API, and any producers still running, survive the old
  handle's cleanup.
- Usage estimates one token per four Unicode scalar values, rounded up. Content
  and prompt separators keep their whitespace. A nonempty session ID enables
  caching when retention is omitted, `Short` or `Long`.
  `Some(CacheRetention::None)` disables both reads and writes and leaves the
  existing cache unchanged. An empty ID counts as absent.
- Tool arguments emit canonical compact JSON deltas in the spelling the authored
  script writes: array-index keys first, and each number as the double it names.
  The usage estimate counts the same text. Partial arguments stay empty until the
  tool ends. Partial observations within one block share its accumulation, but an
  earlier observation you kept never gains later content blocks. Terminal
  messages keep the original content metadata, and constructed partials are
  unsigned.
- Unpaced chunks yield through native async. A positive `tokens_per_second` waits
  for the chunk's estimated tokens divided by that rate. Chunk sizes default to
  three to five tokens. Empty text or thinking still produces one empty delta.
- Cancellation is checked before the start, before each block and after each
  scheduled chunk; it does not race hooks or factories. A cancelled call keeps the
  accumulated content and the full estimated usage, changes the timestamp and
  emits `Request was aborted` in place of done. Dropping event or result
  observers does not stop production.
- Script one flow per registration. Register a separate provider for each
  independent concurrent flow; caches are local to the registration and the
  session.

## Asynchronous factory definitions

`FauxResponseFactory` keeps a named owner that implements `Invoke` for both
`FauxResponseInput<Local>` and `FauxResponseInput<Threaded>`. The future it
selects uses that family's `Execution::Future`. Native owners stay `Send + Sync`
even when they accept local request values. `FauxResponseOptions<E>` aliases
`StreamOptionsInput<E>`, and its raw and simple variants keep their original
fields. A callback written as a four-argument closure becomes one generic
response body plus two concrete future-erasure declarations. See the
[executable factory examples](../agent/api.md#messages-and-conversion).

### `scripted_factory_requires_both_families`

A factory that implements only the threaded invocation cannot be queued:

```compile_fail,E0277
use maestro_models::{
    AssistantMessage, BoxFuture, DiagnosticErrorInfo, FauxResponseFactory,
    FauxResponseInput, Invoke, Threaded,
};
use std::sync::Arc;
struct ThreadedOnly;
impl Invoke<Threaded, FauxResponseInput<Threaded>> for ThreadedOnly {
    type Return = BoxFuture<Result<AssistantMessage, DiagnosticErrorInfo>>;
    fn invoke(&self, _: FauxResponseInput<Threaded>) -> Self::Return {
        Box::pin(async { Err(DiagnosticErrorInfo::error("factory stopped")) })
    }
}
let factory: FauxResponseFactory = Arc::new(ThreadedOnly);
```

## Corrected boundaries and native representation

Prompt tokens split into disjoint input, read and write categories. With caching
on, input is zero and write equals prompt minus read. The total always equals
prompt plus output. This stops cache writes from being charged twice and stops
suffixes from rounding independently. Counting by Unicode scalar boundaries never
splits a supplementary character, and never reuses half of a shared character as a
cached prefix. Separate helpers never share mutable usage defaults.

Compact serde JSON keeps the order in which map keys were inserted (numeric-like
keys included) and the native floating-point spelling. Argument values stay
intact. Opaque random suffixes use native formatting. Generated identifiers keep
three separately created forms: `tool:<milliseconds>:<opaque>`,
`faux:<milliseconds>:<opaque>` and `faux-provider:<milliseconds>:<opaque>`. An
explicit empty identifier stays empty.
