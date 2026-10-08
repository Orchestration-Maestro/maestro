# Scripted faux models

`register_faux_provider` registers a temporary in-memory provider for tests and
demos. It is opt-in, not part of the built-in provider set. All calls use the
ordinary `complete`, `stream` and simple invocation interfaces.

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
let arguments = serde_json::from_value(serde_json::json!({"text": "package.json"}))?;
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

The second turn prints ordered start, thinking, text and done event names. The
last three lines are `true`, `0`, `2`. Run the async example on a caller's executor;
producers use their own timer-enabled runtime natively, or browser-local futures.
No network credentials or server are needed.

## Behavior

- Responses are consumed in request-start FIFO order, including already-aborted
  requests. Exhaustion returns `No more faux responses queued` with estimated
  prompt usage. Factory or response-hook failures instead carry the supplied
  error text and zero usage. The response hook receives status 200 and empty
  headers before factory resolution, even on exhaustion; payload hooks are unused.
- `set_responses` replaces the remaining queue; `append_responses` adds to it.
  Neither resets count or cache. `state.call_count()` is live across async
  factories. Factories receive owned context, raw/simple options and selected model.
- `models` exposes all descriptors; `get_model(None)` or `Some("")` returns the
  first; an unknown nonempty identifier returns `None`. Optional overrides retain
  explicit empty/false/zero values. Empty model lists use the default model.
- `faux_assistant_message` accepts text, a single block or ordered blocks.
  The text/thinking/tool builders supply unsigned content; every message owns fresh
  zero usage. Metadata stays supplied, while invocation rewrites API/provider/model.
- `unregister` removes only this registration's source. Replacing a registration
  on the same API and ongoing producers survive the old handle's cleanup.
- Usage estimates one token per four Unicode scalar values, rounded up. Content
  and prompt separators preserve whitespace. Nonempty session IDs with omitted,
  `Short` or `Long` retention enable caching; `Some(CacheRetention::None)` disables
  both reads and writes without changing existing cache. Empty IDs behave as absent.
- Tool arguments emit compact JSON deltas; partial arguments remain empty until
  tool end. Partial observations within one block share its accumulation, but
  retained earlier observations never gain subsequent content blocks. Terminal
  messages retain original content metadata, while constructed partials are unsigned.
- Unpaced chunks yield through native async; a positive `tokens_per_second` waits
  for the chunk's estimated tokens divided by that rate. Chunk bounds default to
  three–five tokens. Empty text/thinking still produce one empty delta.
- Cancellation is checked before start, before each block and after each scheduled
  chunk, not raced against hooks/factories. It keeps accumulated content and full
  estimated usage, changes the timestamp and emits `Request was aborted` instead
  of done. Dropping event/result observers does not stop production.
- Use one scripted flow per registration. Register separate providers for
  independent concurrent flows; caches are registration-local and session-local.

## Corrected boundaries and native representation

Prompt tokens are partitioned into disjoint input/read/write categories: caching
uses input zero and write equals prompt minus read; total always equals prompt plus
output. This avoids double-charging cache writes and independent suffix rounding.
Unicode scalar boundaries avoid splitting supplementary characters or reusing a
shared half-character as a cached prefix. Independent helpers never share mutable
usage defaults.

Compact serde JSON keeps inserted map order (including numeric-like keys) and
native floating-point spelling. Argument values remain intact. Opaque random
suffixes use native formatting; generated identifiers retain independently
created `tool:<milliseconds>:<opaque>`, `faux:<milliseconds>:<opaque>` and
`faux-provider:<milliseconds>:<opaque>` forms. Explicit empty identifiers stay empty.
