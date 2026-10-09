# Response events

`maestro-models` converts conversation history into response input items and reduces the
service's streamed response events into the shared assistant message. Three functions
carry this, with an options record each:

- `convert_responses_messages(model, context, allowed_tool_call_providers, options)`
  returns the ordered input items for a request.
- `convert_responses_tools(tools, options)` returns the ordered tool declarations.
- `process_responses_stream(events, output, stream, model, options)` reduces event texts
  into `output` and publishes the content updates on `stream`.

They build no request and open no connection. Endpoint selection, headers, caching,
retries and registration belong to the provider that calls them.

## History

`convert_responses_messages` first projects the history for the model (see the
[conversation projection](https://github.com/Orchestration-Maestro/maestro/blob/main/docs/conversation-projection.md)),
then writes one item per user turn, per assistant block and per tool result. The result
is `Vec<serde_json::Value>`.

- The system prompt leads the items when `include_system_prompt` is true (the default)
  and the prompt is not empty. Its role is `developer` for a reasoning model and `system`
  otherwise.
- A user turn becomes one `user` item of `input_text` and `input_image` parts in order;
  images are data URLs. A turn with no blocks produces no item; an empty text is kept.
- A reasoning block with a signature is replayed as the JSON the signature holds, so
  encrypted content and unknown members come back unchanged. A block with no signature, or
  an empty one, produces nothing. A signature that is not JSON, or that nests more than 127 containers,
  fails the conversion with the JSON reader's own message.
- A text block becomes a completed `message` item. The signature gives its identifier and
  phase: version-one JSON with a string `id` and an optional `commentary` or
  `final_answer` phase; any other signature is the identifier itself. An identifier longer
  than 64 UTF-16 units is replaced by `msg_` and a hash of it, and a missing or empty one
  by `msg_` and the number of earlier turns that produced items.
- A tool call becomes a `function_call` item. Its identifier is cut at the first `|` into
  the call identifier and an item identifier; the item identifier is dropped when another
  model of the same provider made a call whose item identifier starts with `fc_`. Arguments
  are compact JSON, array-index keys first, numbers spelled as ECMAScript prints them.
- Tool-call identifiers of other models' turns are normalized when projection asks. Every
  character outside `[A-Za-z0-9_-]` becomes one `_` per UTF-16 unit, a part is cut to 64
  units, and trailing `_` are removed. When the model's provider is in
  `allowed_tool_call_providers` and the identifier holds a `|`, the first two parts are kept;
  an item part from another provider or protocol is replaced by `fc_` and a hash of it, and
  any item part not starting with `fc_` gets that prefix. Otherwise the whole identifier is
  one part.
- A tool result becomes a `function_call_output` with the call identifier only. Its text
  blocks are joined with newlines; when the model reads images, the images follow the text
  as `input_image` parts, and otherwise only the text is sent. A result without text sends
  `(see attached image)`.

`convert_responses_tools` returns `function` declarations with each tool's name,
description and schema untouched. `strict` is `false` by default; `None` sends `null`.

## Events

`process_responses_stream` takes event texts, already framed by the caller, one JSON object
each. It keeps the open item in a small state and the content in `output`, which it shares
with every update: an update's `partial` is `output` itself, so a handle kept from one update
shows later changes.

| Event | Effect |
| --- | --- |
| `response.created` | Takes the response identifier. |
| `response.output_item.added` | Opens a thinking, text or tool-call block and publishes its start. |
| `response.reasoning_summary_part.added` | Notes whether the new part is an object. |
| `response.reasoning_summary_text.delta`, `response.reasoning_summary_part.done` | While the last summary part is an object, add the delta, or `\n\n`, to the thinking. |
| `response.reasoning_text.delta` | Adds the delta to the thinking. |
| `response.content_part.added` | Notes the kind of the part when it is `output_text` or `refusal`. |
| `response.output_text.delta`, `response.refusal.delta` | Add the delta to the text when the last part is of that kind. |
| `response.function_call_arguments.delta` | Adds the delta to the argument text and parses what arrived. |
| `response.function_call_arguments.done` | Replaces the argument text; publishes only the part that extends what arrived. |
| `response.output_item.done` | Replaces the provisional block with the final item and publishes its end. |
| `response.completed`, `response.incomplete` | Take the identifier, usage, cost and outcome. |
| `error`, `response.failed` | Fail the reduction. |

Any other event, an event that is not a JSON object, and a delta that does not fit the open
item are ignored. A member of the wrong type reads as missing; a repeated member keeps its
last value; members no event reads are never decoded, whatever their depth.

At `response.output_item.done` the final item replaces what was streamed:

- Reasoning takes the joined summary parts, else the joined content parts, else keeps the
  streamed thinking. The whole item, compact and with array-index keys first, becomes the
  signature; an item nesting more than 127 containers fails the reduction before the
  block changes.
- A message takes its `output_text` and `refusal` parts joined without a separator. The
  signature is `{"v":1,"id":...}` with the item's identifier and, when present, its phase.
- A function call takes its arguments from the streamed argument text, or from the item
  when nothing streamed. A call the stream never opened is added to the message first, so
  its end is published for the last content position. The tool call keeps only parsed
  arguments; the argument text never reaches the message.

Tool-call arguments are parsed with `parse_streaming_json` and must be an object: any other
value becomes `{}`. Parsing is bounded as that function documents, 127 containers.

`response.completed` and `response.incomplete` read usage, if the response has any:
`input` is `input_tokens` minus the cached tokens, which are `cacheRead`, with no
clamping; absent counts are zero. Without usage the message keeps its counts. Costs are
then recomputed from the model's rates. When `apply_service_tier_pricing` is set it
adjusts the cost, given the tier from `resolve_service_tier(echoed, requested)` or, without
a resolver, the echoed tier, else `service_tier`; the resolver is not called without a
pricing callback. The outcome follows the status: missing, empty, `completed`,
`in_progress` and `queued` mean `Stop`, `incomplete` means `Length`, `failed` and
`cancelled` mean `Error`, and `Stop` becomes `ToolUse` when the message holds a tool call.
Any other status fails with `Unhandled stop reason: {status}`.

## Errors

The reduction returns the first failure and leaves reduced content in `output`:

- A failure of the event source, unchanged.
- Text that is not JSON, or an event whose string holds a lone surrogate: the JSON reader's
  own message.
- `error`: `Error Code {code}: {message}`, a missing member written `undefined` and a `null`
  one `null`.
- `response.failed`: `{code}: {message}` from its error, with `unknown` and `no message` for
  empty members; else `incomplete: {reason}`; else
  `Unknown error (no error details in response)`.
- An unknown status, as above.
- The events ending before a `response.completed` or `response.incomplete`:
  `Response stream ended before a terminal event`.

The reduction never publishes `start`, `done` or `error`, ends `stream`, or applies a time
limit; it continues until the source ends, so an event after a completed response is still
reduced. The caller owns those steps and the final outcome.

## Example

```rust
use std::sync::{Arc, RwLock};

use futures_util::stream;
use maestro_models::{
    AssistantMessage, AssistantMessageEventStream, DiagnosticErrorInfo, Model, StopReason,
    process_responses_stream,
};
use serde_json::json;

let model: Model = serde_json::from_value(json!({
    "id": "controlled", "name": "Controlled", "api": "openai-responses",
    "provider": "controlled", "baseUrl": "https://example.invalid/v1",
    "reasoning": false, "input": ["text"], "contextWindow": 1000, "maxTokens": 100,
    "cost": {"input": 1, "output": 2, "cacheRead": 0, "cacheWrite": 0}
}))?;
let message: AssistantMessage = serde_json::from_value(json!({
    "role": "assistant", "content": [], "api": "openai-responses",
    "provider": "controlled", "model": "controlled", "stopReason": "stop", "timestamp": 0,
    "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
        "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}}
}))?;
let output = Arc::new(RwLock::new(message));
let updates = AssistantMessageEventStream::new();
let events = [
    json!({"type": "response.output_item.added", "item": {"type": "message", "id": "msg_1",
        "role": "assistant", "status": "in_progress", "content": []}}),
    json!({"type": "response.content_part.added", "part": {"type": "output_text", "text": ""}}),
    json!({"type": "response.output_text.delta", "delta": "hi"}),
    json!({"type": "response.output_item.done", "item": {"type": "message", "id": "msg_1",
        "role": "assistant", "status": "completed",
        "content": [{"type": "output_text", "text": "hi"}]}}),
    json!({"type": "response.completed", "response": {"id": "resp_1", "status": "completed",
        "usage": {"input_tokens": 3, "output_tokens": 1, "total_tokens": 4}}}),
]
.map(|event| Ok::<_, DiagnosticErrorInfo>(event.to_string()));

let runtime = tokio::runtime::Builder::new_current_thread().build()?;
runtime.block_on(process_responses_stream(
    stream::iter(events),
    &output,
    &updates,
    &model,
    None,
))?;

let message = output.read().map_err(|error| error.to_string())?;
assert_eq!(message.stop_reason, StopReason::Stop);
assert_eq!(message.response_id.as_deref(), Some("resp_1"));
assert_eq!((message.usage.input, message.usage.output), (3.0, 1.0));
# Ok::<(), Box<dyn std::error::Error>>(())
```
